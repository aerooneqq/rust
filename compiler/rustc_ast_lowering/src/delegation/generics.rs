use hir::HirId;
use hir::def::{DefKind, Res};
use rustc_ast::*;
use rustc_hir as hir;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_middle::ty::{
    self, GenericParamDefKind, Ty, TyCtxt, TypeFoldable, TypeFolder, TypeSuperFoldable,
    TypeVisitableExt,
};
use rustc_span::sym::{self};
use rustc_span::symbol::kw;
use rustc_span::{DUMMY_SP, Ident, Span, Symbol};
use thin_vec::thin_vec;

use crate::delegation::delegation::DelegationIds;
use crate::{AstOwner, LoweringContext};

struct DelegationArgsInfo {
    generate_self: bool,
    can_add_generics_to_parent: bool,
    parent_user_specified: bool,
    child_user_specified: bool,
    propagate_self_ty: bool,
}

impl<'hir> LoweringContext<'_, 'hir> {
    pub(super) fn lower_delegation_generics(
        &mut self,
        _delegation: &Delegation,
        ids: &DelegationIds,
        item_id: NodeId,
        span: Span,
    ) -> GenericsGenerationResults<'hir> {
        let mut prev_child_generics = None;
        let mut parent;
        let mut child;

        let (param_count, _) = self.param_count(ids.root_function_id());
        let mut selfs_count = 0;

        for i in (2..ids.path.len() + 1).rev() {
            let [delegation_id, sig_id] = ids.path[i - 2..i] else { unreachable!() };
            let delegation_id = delegation_id.expect_local();
            let is_method = self.is_delegation_a_method(&ids.path[i - 1..], span);

            (parent, child) = self.do_step(delegation_id, sig_id, is_method, prev_child_generics);

            let will_generate_method_call = self.can_generate_method_call(
                self.get_delegation(delegation_id),
                is_method,
                ids.root_function_id(),
                sig_id,
                param_count,
                delegation_id,
            );

            let empty_parent = DelegationGenerics::empty_generics(DelegationGenericsKind::Default);

            let mut new_generics = Self::create_generics(
                if will_generate_method_call { &empty_parent } else { &parent },
                &child,
            );

            for p in &mut new_generics.generics.params {
                if p.ident.name == kw::SelfUpper {
                    let name = format!("{}{}", kw::SelfUpper, selfs_count + 1);
                    p.ident.name = Symbol::intern(name.as_str());
                    selfs_count += 1;
                }
            }

            prev_child_generics = Some(new_generics);
        }

        let (delegation_id, sig_id) = (self.local_def_id(item_id), ids.delegee_id());
        let is_method = self.is_delegation_a_method(&ids.path, span);

        (parent, child) = self.do_step(delegation_id, sig_id, is_method, prev_child_generics);

        let args_info = self.get_delegation_args_info(delegation_id, sig_id, is_method);

        GenericsGenerationResults {
            parent: GenericsGenerationResult::new(parent),
            child: GenericsGenerationResult::new(child),
            self_ty_id: None,
            propagate_self_ty: args_info.propagate_self_ty,
        }
    }

    fn do_step(
        &mut self,
        delegation_id: LocalDefId,
        sig_id: DefId,
        is_method: bool,
        prev_child_generics: Option<AstGenerics>,
    ) -> (DelegationGenerics<AstGenerics>, DelegationGenerics<AstGenerics>) {
        let args_info = self.get_delegation_args_info(delegation_id, sig_id, is_method);

        let parent_generics = if args_info.can_add_generics_to_parent {
            if args_info.parent_user_specified {
                if args_info.generate_self {
                    DelegationGenerics::new(
                        self.get_parent_generics(
                            self.tcx.opt_parent(sig_id),
                            args_info.generate_self,
                            true,
                        ),
                        DelegationGenericsKind::SelfAndUserSpecified,
                    )
                } else {
                    DelegationGenerics::empty_generics(DelegationGenericsKind::UserSpecified)
                }
            } else {
                DelegationGenerics::new(
                    self.get_parent_generics(
                        self.tcx.opt_parent(sig_id),
                        args_info.generate_self,
                        false,
                    ),
                    DelegationGenericsKind::Default,
                )
            }
        } else {
            DelegationGenerics::empty_generics(DelegationGenericsKind::Default)
        };

        let child_generics = if args_info.child_user_specified {
            DelegationGenerics::new(
                self.create_synth_params_only_ast_generics(sig_id),
                DelegationGenericsKind::UserSpecified,
            )
        } else {
            DelegationGenerics::new(
                prev_child_generics.or_else(|| self.get_fn_like_generics(sig_id)),
                DelegationGenericsKind::Default,
            )
        };

        (parent_generics, child_generics)
    }

    fn create_generics(
        parent_generics: &DelegationGenerics<AstGenerics>,
        child_generics: &DelegationGenerics<AstGenerics>,
    ) -> AstGenerics {
        AstGenerics {
            generics: Generics {
                params: parent_generics
                    .all_lifetimes()
                    .chain(child_generics.all_lifetimes())
                    .chain(parent_generics.all_types_and_consts())
                    .chain(child_generics.all_types_and_consts())
                    .map(|p| p.clone())
                    .collect(),
                ..Default::default()
            },
            synthetic_params: child_generics.all_synthetic_names().map(|s| *s).collect(),
        }
    }

    fn get_delegation_args_info(
        &self,
        delegation_id: LocalDefId,
        sig_id: DefId,
        is_method: bool,
    ) -> DelegationArgsInfo {
        let delegation = self.get_delegation(delegation_id);
        let free_to_trait_delegation = self.is_free_to_trait_reuse(delegation_id, sig_id);
        let generate_self = free_to_trait_delegation && is_method && delegation.qself.is_none();

        let segments = &delegation.path.segments;
        let len = segments.len();

        DelegationArgsInfo {
            generate_self,
            can_add_generics_to_parent: len >= 2 && self.can_add_generics_to(segments[len - 2].id),
            parent_user_specified: len >= 2 && segments[len - 2].args.is_some(),
            child_user_specified: segments[len - 1].args.is_some(),
            propagate_self_ty: free_to_trait_delegation && !generate_self,
        }
    }

    fn create_synth_params_only_ast_generics(&self, id: DefId) -> Option<AstGenerics> {
        if let Some(local_id) = id.as_local() {
            Some(AstGenerics {
                generics: Default::default(),
                synthetic_params: self.get_synthetic_params_symbols(local_id),
            })
        } else {
            self.get_external_synth_only_generics(id)
        }
    }

    fn get_delegation(&self, delegation_id: LocalDefId) -> &Delegation {
        match self.ast_accessor.get(delegation_id).unwrap() {
            AstOwner::Item(item) if let ItemKind::Delegation(d) = &item.kind => d.as_ref(),
            AstOwner::AssocItem(item, _) if let AssocItemKind::Delegation(d) = &item.kind => {
                d.as_ref()
            }
            _ => unreachable!(),
        }
    }

    pub(super) fn is_free_to_trait_reuse(&self, delegation_id: LocalDefId, sig_id: DefId) -> bool {
        let delegation_in_free_ctx = self.is_free_ctx(delegation_id.into());

        let root_function_in_trait = self
            .tcx
            .opt_parent(sig_id)
            .is_some_and(|p| matches!(self.tcx.def_kind(p), DefKind::Trait));

        delegation_in_free_ctx && root_function_in_trait
    }

    pub(super) fn is_free_ctx(&self, id: DefId) -> bool {
        self.tcx
            .opt_parent(id)
            .is_none_or(|p| !matches!(self.tcx.def_kind(p), DefKind::Trait | DefKind::Impl { .. }))
    }

    fn can_add_generics_to(&self, node_id: NodeId) -> bool {
        self.get_resolution_id(node_id).is_some_and(|def_id| {
            matches!(self.tcx.def_kind(def_id), DefKind::Trait | DefKind::TraitAlias)
        })
    }

    fn lower_delegation_generic_params(
        &mut self,
        item_id: NodeId,
        span: Span,
        generics: &AstGenerics,
    ) -> &'hir hir::Generics<'hir> {
        let mut params = generics.generics.params.clone();

        for p in &mut params {
            // We want to create completely new params, so we generate
            // a new id, otherwise assertions will be triggered.
            p.id = self.next_node_id();

            // Remove default params, as they are not supported on functions
            // and there will duplicate DefId  when we try to lower them later.
            match &mut p.kind {
                GenericParamKind::Lifetime => {}
                GenericParamKind::Type { default } => *default = None,
                GenericParamKind::Const { default, .. } => *default = None,
            }

            self.create_generic_param_def_id(
                item_id,
                p.id,
                p.ident.name,
                match p.kind {
                    GenericParamKind::Lifetime => DefKind::LifetimeParam,
                    GenericParamKind::Type { .. } => DefKind::TyParam,
                    GenericParamKind::Const { .. } => DefKind::ConstParam,
                },
            );
        }

        let synth_params = generics
            .synthetic_params
            .iter()
            .map(|name| self.create_hir_synthetic_generic_param(item_id, *name))
            .collect::<Vec<_>>();

        // Fallback to default generic param lowering, we modified them in the loop above.
        let params = self.arena.alloc_from_iter(
            params
                .iter()
                .map(|p| self.lower_generic_param(p, hir::GenericParamSource::Generics))
                .chain(synth_params),
        );

        // HACK: for now we generate predicates such that all lifetimes are early bound,
        // we can not not generate early-bound lifetimes, but we can't know which of them
        // are late-bound at this level of compilation.
        // FIXME(fn_delegation): proper support for late bound lifetimes.
        self.arena.alloc(hir::Generics {
            params,
            predicates: self.arena.alloc_from_iter(
                params
                    .iter()
                    .filter_map(|p| p.is_lifetime().then(|| self.generate_lifetime_predicate(p))),
            ),
            has_where_clause_predicates: false,
            where_clause_span: span,
            span,
        })
    }

    fn create_generic_param_def_id(
        &mut self,
        item_id: NodeId,
        node_id: NodeId,
        name: Symbol,
        def_kind: DefKind,
    ) -> LocalDefId {
        // Note that we use self.disambiguator here, if we will create new every time
        // we will get ICE if params have the same name.
        let def_id = self
            .tcx
            .create_def(
                self.resolver.node_id_to_def_id[&item_id],
                Some(name),
                def_kind,
                None,
                &mut self.disambiguator,
            )
            .def_id();

        self.resolver.node_id_to_def_id.insert(node_id, def_id);

        def_id
    }

    fn create_hir_synthetic_generic_param(
        &mut self,
        item_id: NodeId,
        name: Symbol,
    ) -> hir::GenericParam<'hir> {
        let node_id = self.next_node_id();
        let def_id = self.create_generic_param_def_id(item_id, node_id, name, DefKind::TyParam);

        hir::GenericParam {
            hir_id: self.lower_node_id(node_id),
            def_id,
            name: hir::ParamName::Plain(Ident::with_dummy_span(name)),
            pure_wrt_drop: false,
            span: DUMMY_SP,
            kind: hir::GenericParamKind::Type { default: None, synthetic: true },
            colon_span: None,
            source: hir::GenericParamSource::Generics,
        }
    }

    fn generate_lifetime_predicate(
        &mut self,
        p: &hir::GenericParam<'hir>,
    ) -> hir::WherePredicate<'hir> {
        let create_lifetime = |this: &mut Self| -> &'hir hir::Lifetime {
            this.arena.alloc(hir::Lifetime {
                hir_id: this.next_id(),
                ident: p.name.ident(),
                kind: rustc_hir::LifetimeKind::Param(p.def_id),
                source: rustc_hir::LifetimeSource::Path {
                    angle_brackets: rustc_hir::AngleBrackets::Full,
                },
                syntax: rustc_hir::LifetimeSyntax::ExplicitBound,
            })
        };

        hir::WherePredicate {
            hir_id: self.next_id(),
            span: DUMMY_SP,
            kind: self.arena.alloc(hir::WherePredicateKind::RegionPredicate(
                hir::WhereRegionPredicate {
                    in_where_clause: true,
                    lifetime: create_lifetime(self),
                    bounds: self
                        .arena
                        .alloc_slice(&[hir::GenericBound::Outlives(create_lifetime(self))]),
                },
            )),
        }
    }

    fn create_generics_args_from_params(
        &mut self,
        params: &[hir::GenericParam<'hir>],
        add_lifetimes: bool,
        span: Span,
    ) -> &'hir hir::GenericArgs<'hir> {
        self.arena.alloc(hir::GenericArgs {
            args: self.arena.alloc_from_iter(params.iter().filter_map(|p| {
                // Skip self generic arg or synthetic args, we do not need to propagate them.
                if p.name.ident().name == kw::SelfUpper || p.is_impl_trait() {
                    return None;
                }

                let create_path = |this: &mut Self| {
                    let res = Res::Def(
                        match p.kind {
                            hir::GenericParamKind::Lifetime { .. } => DefKind::LifetimeParam,
                            hir::GenericParamKind::Type { .. } => DefKind::TyParam,
                            hir::GenericParamKind::Const { .. } => DefKind::ConstParam,
                        },
                        p.def_id.to_def_id(),
                    );

                    hir::QPath::Resolved(
                        None,
                        self.arena.alloc(hir::Path {
                            segments: this.arena.alloc_slice(&[hir::PathSegment {
                                args: None,
                                hir_id: this.next_id(),
                                ident: p.name.ident(),
                                infer_args: false,
                                res,
                            }]),
                            res,
                            span: p.span,
                        }),
                    )
                };

                match p.kind {
                    hir::GenericParamKind::Lifetime { .. } => match add_lifetimes {
                        true => Some(hir::GenericArg::Lifetime(self.arena.alloc(hir::Lifetime {
                            hir_id: self.next_id(),
                            ident: p.name.ident(),
                            kind: hir::LifetimeKind::Param(p.def_id),
                            source: hir::LifetimeSource::Path {
                                angle_brackets: hir::AngleBrackets::Full,
                            },
                            syntax: hir::LifetimeSyntax::ExplicitBound,
                        }))),
                        false => None,
                    },
                    hir::GenericParamKind::Type { .. } => {
                        Some(hir::GenericArg::Type(self.arena.alloc(hir::Ty {
                            hir_id: self.next_id(),
                            span: p.span,
                            kind: hir::TyKind::Path(create_path(self)),
                        })))
                    }
                    hir::GenericParamKind::Const { .. } => {
                        Some(hir::GenericArg::Const(self.arena.alloc(hir::ConstArg {
                            hir_id: self.next_id(),
                            kind: hir::ConstArgKind::Path(create_path(self)),
                            span: p.span,
                        })))
                    }
                }
            })),
            constraints: &[],
            parenthesized: hir::GenericArgsParentheses::No,
            span_ext: span,
        })
    }

    fn get_fn_like_generics(&mut self, id: DefId) -> Option<AstGenerics> {
        if let Some(local_id) = id.as_local() {
            self.get_fn(local_id).map(|f| AstGenerics {
                generics: f.generics.clone(),
                synthetic_params: self.get_synthetic_params_symbols(local_id),
            })
        } else {
            self.get_external_generics(id, false)
        }
    }

    fn get_synthetic_params_symbols(&self, local_id: LocalDefId) -> Vec<Symbol> {
        self.hir_accessor
            .generics_of(local_id)
            .map(|g| {
                g.params
                    .iter()
                    .filter_map(
                        |p| if p.is_impl_trait() { Some(p.name.ident().name) } else { None },
                    )
                    .collect()
            })
            .unwrap_or(vec![])
    }

    pub(super) fn get_fn(&self, local_id: LocalDefId) -> Option<&Box<Fn>> {
        match self.ast_accessor.get(local_id) {
            Some(AstOwner::Item(item)) if let ItemKind::Fn(f) = &item.kind => Some(f),
            Some(AstOwner::AssocItem(item, _)) if let AssocItemKind::Fn(f) = &item.kind => Some(f),
            _ => None,
        }
    }

    fn get_external_synth_only_generics(&self, id: DefId) -> Option<AstGenerics> {
        let generics = self.tcx.generics_of(id);
        if generics.own_params.is_empty() {
            return None;
        }

        Some(AstGenerics {
            generics: Default::default(),
            synthetic_params: generics
                .own_params
                .iter()
                .filter(|p| p.kind.is_synthetic())
                .map(|p| p.name)
                .collect(),
        })
    }

    fn get_external_generics(&mut self, id: DefId, processing_parent: bool) -> Option<AstGenerics> {
        let generics = self.tcx.generics_of(id);
        if generics.own_params.is_empty() {
            return None;
        }

        // Skip first Self parameter if we are in trait, it will be added later.
        let to_skip = (processing_parent && generics.has_self) as usize;

        let mut params = thin_vec![];
        let mut synth_idents = vec![];

        for param in generics.own_params.iter().skip(to_skip) {
            if param.kind.is_synthetic() {
                synth_idents.push(param.name);
            } else {
                params.push(GenericParam {
                    attrs: Default::default(),
                    bounds: Default::default(),
                    colon_span: None,
                    id: self.next_node_id(),
                    ident: Ident::with_dummy_span(param.name),
                    is_placeholder: false,
                    kind: match param.kind {
                        GenericParamDefKind::Lifetime => GenericParamKind::Lifetime,
                        GenericParamDefKind::Type { .. } => {
                            GenericParamKind::Type { default: None }
                        }
                        GenericParamDefKind::Const { .. } => self.map_const_kind(param),
                    },
                });
            }
        }

        Some(AstGenerics {
            generics: Generics { params, where_clause: Default::default(), span: DUMMY_SP },
            synthetic_params: synth_idents,
        })
    }

    fn map_const_kind(&mut self, p: &ty::GenericParamDef) -> GenericParamKind {
        let const_type = self.tcx.type_of(p.def_id).instantiate_identity().kind();

        let (type_symbol, res) = match const_type {
            ty::Bool => (sym::bool, Res::PrimTy(hir::PrimTy::Bool)),
            ty::Uint(uint) => (uint.name(), Res::PrimTy(hir::PrimTy::Uint(*uint))),
            ty::Int(int) => (int.name(), Res::PrimTy(hir::PrimTy::Int(*int))),
            ty::Char => (sym::char, Res::PrimTy(hir::PrimTy::Char)),
            _ => (sym::dummy, Res::Err),
        };

        let node_id = self.next_node_id();

        self.resolver.partial_res_map.insert(node_id, hir::def::PartialRes::new(res));

        GenericParamKind::Const {
            ty: Box::new(rustc_ast::Ty {
                id: node_id,
                kind: TyKind::Path(
                    None,
                    Path {
                        segments: thin_vec![PathSegment {
                            ident: Ident::with_dummy_span(type_symbol),
                            id: self.next_node_id(),
                            args: None
                        }],
                        span: DUMMY_SP,
                        tokens: None,
                    },
                ),
                span: DUMMY_SP,
                tokens: None,
            }),
            span: DUMMY_SP,
            default: None,
        }
    }

    fn get_parent_generics(
        &mut self,
        id: Option<DefId>,
        add_self: bool,
        user_specified: bool,
    ) -> Option<AstGenerics> {
        let id = if let Some(id) = id { id } else { return None };

        // If args are user-specified we still maybe need to add self
        let mut generics = if user_specified {
            None
        } else {
            if let Some(local_id) = id.as_local() {
                if let Some(AstOwner::Item(item)) = self.ast_accessor.get(local_id)
                    && matches!(item.kind, ItemKind::Trait(..))
                {
                    item.opt_generics().cloned().map(|generics| AstGenerics {
                        generics,
                        synthetic_params: self.get_synthetic_params_symbols(local_id),
                    })
                } else {
                    None
                }
            } else {
                self.get_external_generics(id, true)
            }
        };

        if add_self {
            generics = Some(generics.unwrap_or_default());

            generics.as_mut().unwrap().generics.params.insert(
                0,
                GenericParam {
                    id: self.next_node_id(),
                    ident: Ident::new(kw::SelfUpper, DUMMY_SP),
                    attrs: Default::default(),
                    bounds: vec![],
                    is_placeholder: false,
                    kind: GenericParamKind::Type { default: None },
                    colon_span: None,
                },
            );
        }

        generics
    }

    pub(super) fn references_parent_generics_external(&self, root_fn_id: DefId) -> bool {
        struct ParentGenericParamsUsageMarker<'tcx> {
            tcx: TyCtxt<'tcx>,
            parent_count: u32,
            references_parent_generics: bool,
        }

        impl<'tcx> TypeFolder<TyCtxt<'tcx>> for ParentGenericParamsUsageMarker<'tcx> {
            fn cx(&self) -> TyCtxt<'tcx> {
                self.tcx
            }

            fn fold_ty(&mut self, ty: Ty<'tcx>) -> Ty<'tcx> {
                if !ty.has_param() {
                    return ty;
                }

                if let ty::Param(param) = ty.kind() {
                    self.check_if_parent_param(param.index);
                }

                ty.super_fold_with(self)
            }

            fn fold_region(&mut self, r: ty::Region<'tcx>) -> ty::Region<'tcx> {
                if let ty::ReEarlyParam(param) = r.kind() {
                    self.check_if_parent_param(param.index);
                }

                r
            }

            fn fold_const(&mut self, ct: ty::Const<'tcx>) -> ty::Const<'tcx> {
                if let ty::ConstKind::Param(param) = ct.kind() {
                    self.check_if_parent_param(param.index);
                }

                ct.super_fold_with(self)
            }
        }

        impl<'tcx> ParentGenericParamsUsageMarker<'tcx> {
            fn check_if_parent_param(&mut self, index: u32) {
                // Don't consider usage of Self
                self.references_parent_generics |= index > 0 && index < self.parent_count;
            }
        }

        let generics = self.tcx.generics_of(root_fn_id);

        let mut marker = ParentGenericParamsUsageMarker {
            parent_count: generics.parent_count as u32,
            references_parent_generics: false,
            tcx: self.tcx,
        };

        self.tcx.fn_sig(root_fn_id).skip_binder().fold_with(&mut marker);
        for (pred, _) in self.tcx.predicates_of(root_fn_id).predicates {
            pred.fold_with(&mut marker);
        }

        marker.references_parent_generics
    }
}

#[derive(Debug, Default)]
pub(super) struct AstGenerics {
    generics: Generics,
    synthetic_params: Vec<Symbol>,
}

pub(super) enum HirOrAstGenerics<'hir> {
    Ast(DelegationGenerics<AstGenerics>),
    Hir(DelegationGenerics<&'hir hir::Generics<'hir>>),
}

impl<'hir> HirOrAstGenerics<'hir> {
    pub(super) fn into_hir_generics(
        &mut self,
        ctx: &mut LoweringContext<'_, 'hir>,
        item_id: NodeId,
        span: Span,
    ) -> &mut Self {
        match self {
            HirOrAstGenerics::Ast(generics) => {
                *self = Self::to_hir_generics(generics, ctx, item_id, span);
            }
            HirOrAstGenerics::Hir(_) => {}
        }

        self
    }

    fn to_hir_generics(
        generics: &DelegationGenerics<AstGenerics>,
        ctx: &mut LoweringContext<'_, 'hir>,
        item_id: NodeId,
        span: Span,
    ) -> Self {
        Self::Hir(
            generics.map(|generics| ctx.lower_delegation_generic_params(item_id, span, generics)),
        )
    }

    pub(super) fn into_hir_generics_self_user_specified_only(
        &mut self,
        ctx: &mut LoweringContext<'_, 'hir>,
        item_id: NodeId,
        span: Span,
    ) -> &mut Self {
        match self {
            HirOrAstGenerics::Ast(generics)
                if matches!(generics.kind, DelegationGenericsKind::SelfAndUserSpecified) =>
            {
                *self = Self::to_hir_generics(generics, ctx, item_id, span);
            }
            _ => {}
        }

        self
    }

    fn hir_generics_or_empty(&self) -> &'hir hir::Generics<'hir> {
        match self {
            HirOrAstGenerics::Ast(_) => hir::Generics::empty(),
            HirOrAstGenerics::Hir(hir_generics) => {
                hir_generics.generics.as_ref().unwrap_or(&hir::Generics::empty())
            }
        }
    }

    pub(super) fn into_generic_args(
        &self,
        ctx: &mut LoweringContext<'_, 'hir>,
        add_lifetimes: bool,
        span: Span,
    ) -> Option<&'hir hir::GenericArgs<'hir>> {
        match self {
            HirOrAstGenerics::Ast(_) => None,
            HirOrAstGenerics::Hir(hir_generics) => hir_generics.generics.map(|generics| {
                ctx.create_generics_args_from_params(generics.params, add_lifetimes, span)
            }),
        }
    }

    pub(super) fn is_user_specified(&self) -> bool {
        match self {
            HirOrAstGenerics::Ast(ast_generics) => ast_generics.is_user_specified(),
            HirOrAstGenerics::Hir(hir_generics) => hir_generics.is_user_specified(),
        }
    }
}

pub(super) struct GenericsGenerationResult<'hir, T: From<HirId>> {
    pub(super) generics: HirOrAstGenerics<'hir>,
    pub(super) args_segment_id: Option<T>,
}

impl<'a, T: From<HirId>> GenericsGenerationResult<'a, T> {
    fn new(generics: DelegationGenerics<AstGenerics>) -> Self {
        Self { generics: HirOrAstGenerics::Ast(generics), args_segment_id: None }
    }
}

pub(super) struct GenericsGenerationResults<'hir> {
    pub(super) parent: GenericsGenerationResult<'hir, hir::DelegationParentGenerics>,
    pub(super) child: GenericsGenerationResult<'hir, HirId>,
    pub(super) self_ty_id: Option<HirId>,
    pub(super) propagate_self_ty: bool,
}

impl<'hir> GenericsGenerationResults<'hir> {
    pub(super) fn all_params(
        &mut self,
        item_id: NodeId,
        span: Span,
        ctx: &mut LoweringContext<'_, 'hir>,
    ) -> impl Iterator<Item = hir::GenericParam<'hir>> {
        let parent = self
            .parent
            .generics
            .into_hir_generics_self_user_specified_only(ctx, item_id, span)
            .hir_generics_or_empty()
            .params;

        let child = self
            .child
            .generics
            .into_hir_generics(ctx, item_id, span)
            .hir_generics_or_empty()
            .params;

        // Order generics, firstly we have parent and child lifetimes,
        // then parent and child types and consts.
        // `generics_of` in `rustc_hir_analysis` will order them anyway,
        // however we want the order to be consistent in HIR too.
        parent
            .iter()
            .filter(|p| p.is_lifetime())
            .chain(child.iter().filter(|p| p.is_lifetime()))
            .chain(parent.iter().filter(|p| !p.is_lifetime()))
            .chain(child.iter().filter(|p| !p.is_lifetime()))
            .map(|p| *p)
    }

    pub(super) fn all_predicates(&self) -> impl Iterator<Item = hir::WherePredicate<'hir>> {
        self.parent
            .generics
            .hir_generics_or_empty()
            .predicates
            .into_iter()
            .chain(self.child.generics.hir_generics_or_empty().predicates.into_iter())
            .map(|p| *p)
    }

    pub(super) fn create_hir_delegation_generics(&self) -> hir::DelegationGenerics {
        hir::DelegationGenerics {
            child_args_segment_id: self.child.args_segment_id,
            parent_args_segment_id: self.parent.args_segment_id,
            self_ty_id: self.self_ty_id,
            propagate_self_ty: self.propagate_self_ty,
        }
    }
}

#[derive(Debug)]
pub(super) struct DelegationGenerics<T> {
    generics: Option<T>,
    kind: DelegationGenericsKind,
}

impl<T> DelegationGenerics<T> {
    fn new(generics: Option<T>, kind: DelegationGenericsKind) -> Self {
        Self { generics, kind }
    }

    fn empty_generics(kind: DelegationGenericsKind) -> Self {
        Self::new(None, kind)
    }

    fn map<U>(&self, f: impl FnOnce(&T) -> U) -> DelegationGenerics<U> {
        DelegationGenerics::<U> { generics: self.generics.as_ref().map(f), kind: self.kind }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum DelegationGenericsKind {
    UserSpecified,
    Default,
    SelfAndUserSpecified,
}

impl DelegationGenerics<AstGenerics> {
    fn all_lifetimes(&self) -> impl Iterator<Item = &GenericParam> {
        self.all_params().filter(|p| p.kind.is_lifetime())
    }

    fn all_types_and_consts(&self) -> impl Iterator<Item = &GenericParam> {
        self.all_params().filter(|p| !p.kind.is_lifetime())
    }

    fn all_params(&self) -> impl Iterator<Item = &GenericParam> {
        self.generics.as_ref().map(|g| g.generics.params.as_slice()).unwrap_or(&[]).iter()
    }

    fn all_synthetic_names(&self) -> impl Iterator<Item = &Symbol> {
        self.generics.as_ref().map(|g| g.synthetic_params.as_slice()).unwrap_or(&[]).iter()
    }
}

impl<T> DelegationGenerics<T> {
    fn is_user_specified(&self) -> bool {
        matches!(
            self.kind,
            DelegationGenericsKind::UserSpecified | DelegationGenericsKind::SelfAndUserSpecified
        )
    }
}
