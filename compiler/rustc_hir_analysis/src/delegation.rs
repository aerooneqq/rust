//! Support inheriting generic parameters and predicates for function delegation.
//!
//! For more information about delegation design, see the tracking issue #118212.

use rustc_data_structures::debug_assert_matches;
use rustc_data_structures::fx::FxHashMap;
use rustc_hir::DelegationGenerics;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_middle::ty::{
    self, EarlyBinder, GenericPredicates, Ty, TyCtxt, TypeFoldable, TypeFolder, TypeSuperFoldable,
    TypeVisitableExt,
};
use rustc_span::{ErrorGuaranteed, Span};

use crate::collect::ItemCtxt;

type RemapTable = FxHashMap<u32, u32>;

struct ParamIndexRemapper<'tcx> {
    tcx: TyCtxt<'tcx>,
    remap_table: RemapTable,
}

impl<'tcx> TypeFolder<TyCtxt<'tcx>> for ParamIndexRemapper<'tcx> {
    fn cx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn fold_ty(&mut self, ty: Ty<'tcx>) -> Ty<'tcx> {
        if !ty.has_param() {
            return ty;
        }

        if let ty::Param(param) = ty.kind()
            && let Some(index) = self.remap_table.get(&param.index)
        {
            return Ty::new_param(self.tcx, *index, param.name);
        }
        ty.super_fold_with(self)
    }

    fn fold_region(&mut self, r: ty::Region<'tcx>) -> ty::Region<'tcx> {
        if let ty::ReEarlyParam(param) = r.kind()
            && let Some(index) = self.remap_table.get(&param.index).copied()
        {
            return ty::Region::new_early_param(
                self.tcx,
                ty::EarlyParamRegion { index, name: param.name },
            );
        }
        r
    }

    fn fold_const(&mut self, ct: ty::Const<'tcx>) -> ty::Const<'tcx> {
        if let ty::ConstKind::Param(param) = ct.kind()
            && let Some(idx) = self.remap_table.get(&param.index)
        {
            let param = ty::ParamConst::new(*idx, param.name);
            return ty::Const::new_param(self.tcx, param);
        }
        ct.super_fold_with(self)
    }
}

#[derive(Debug)]
enum PropagateSelfTy {
    Yes,
    No,
}

#[derive(Debug)]
enum SelfPositionKind {
    AfterLifetimes(PropagateSelfTy),
    Zero,
    None,
}

macro_rules! unsupported_caller_callee_kinds {
    () => {
        // For trait impl's `sig_id` is always equal to the corresponding trait method.
        // For inherent methods delegation is not yet supported.
        (FnKind::AssocTraitImpl, _) | (_, FnKind::AssocTraitImpl) | (_, FnKind::AssocInherentImpl)
    };
}

fn create_self_position_kind(
    tcx: TyCtxt<'_>,
    def_id: LocalDefId,
    sig_id: DefId,
) -> SelfPositionKind {
    match get_caller_and_callee_kind(tcx, def_id, sig_id) {
        (FnKind::AssocInherentImpl, FnKind::AssocTrait)
        | (FnKind::AssocTraitImpl, FnKind::AssocTrait)
        | (FnKind::AssocTrait, FnKind::AssocTrait)
        | (FnKind::AssocTrait, FnKind::Free) => SelfPositionKind::Zero,

        (FnKind::Free, FnKind::AssocTrait) => {
            SelfPositionKind::AfterLifetimes(should_propagate_self_ty(tcx, def_id))
        }

        _ => SelfPositionKind::None,
    }
}

fn should_propagate_self_ty(tcx: TyCtxt<'_>, delegation_id: LocalDefId) -> PropagateSelfTy {
    if get_delegation_generics_info(tcx, delegation_id).propagate_self_ty {
        PropagateSelfTy::Yes
    } else {
        PropagateSelfTy::No
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FnKind {
    Free,
    AssocInherentImpl,
    AssocTrait,
    AssocTraitImpl,
}

fn fn_kind<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> FnKind {
    debug_assert_matches!(tcx.def_kind(def_id), DefKind::Fn | DefKind::AssocFn);

    let parent = tcx.parent(def_id);
    match tcx.def_kind(parent) {
        DefKind::Trait => FnKind::AssocTrait,
        DefKind::Impl { of_trait: true } => FnKind::AssocTraitImpl,
        DefKind::Impl { of_trait: false } => FnKind::AssocInherentImpl,
        _ => FnKind::Free,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SelfPredsFilterKind {
    None,
    OnlyExplicitTrait,
    All,
}

/// Given the current context(caller and callee `FnKind`), it specifies
/// the policy of predicates and generic parameters inheritance.
#[derive(Clone, Copy, Debug, PartialEq)]
enum InheritanceKind {
    /// Copying all predicates and parameters, including those of the parent
    /// container.
    ///
    /// Boolean value defines whether the `Self` parameter or `Self: Trait`
    /// predicate are copied. It's always equal to `false` except when
    /// delegating from a free function to a trait method.
    ///
    /// FIXME(fn_delegation): This often leads to type inference
    /// errors. Support providing generic arguments or restrict use sites.
    WithParent(SelfPredsFilterKind),
    /// The trait implementation should be compatible with the original trait.
    /// Therefore, for trait implementations only the method's own parameters
    /// and predicates are copied.
    Own,
}

impl InheritanceKind {
    fn filter_all_self_preds(&self) -> bool {
        matches!(self, Self::WithParent(SelfPredsFilterKind::All))
    }
}

/// Maps sig generics into generic args of delegation. Delegation generics has the following pattern:
///
/// [SELF | maybe self in the beginning]
/// [PARENT | args of delegation parent]
/// [SIG PARENT LIFETIMES]
/// [SIG LIFETIMES]
/// [SELF | maybe self after lifetimes, when we reuse trait fn in free context]
/// [SIG PARENT TYPES/CONSTS]
/// [SIG TYPES/CONSTS]
fn create_mapping<'tcx>(
    tcx: TyCtxt<'tcx>,
    sig_id: DefId,
    def_id: LocalDefId,
    args: &Vec<ty::GenericArg<'tcx>>,
) -> FxHashMap<u32, u32> {
    let mut mapping: FxHashMap<u32, u32> = Default::default();

    let (_, callee_kind) = get_caller_and_callee_kind(tcx, def_id, sig_id);
    let self_pos_kind = create_self_position_kind(tcx, def_id, sig_id);
    let is_self_at_zero = matches!(self_pos_kind, SelfPositionKind::Zero);

    // Is self at zero? If so insert mapping, self in sig parent is always at 0.
    if is_self_at_zero {
        mapping.insert(0, 0);
    }

    let mut args_index = 0;

    args_index += is_self_at_zero as usize;
    args_index += get_delegation_parent_args_count_without_self(tcx, def_id, sig_id);

    let sig_generics = tcx.generics_of(sig_id);
    let process_sig_parent_generics = matches!(callee_kind, FnKind::AssocTrait);

    if process_sig_parent_generics {
        for i in (sig_generics.has_self as usize)..sig_generics.parent_count {
            let param = sig_generics.param_at(i, tcx);
            if !param.kind.is_ty_or_const() {
                mapping.insert(param.index, args_index as u32);
                args_index += 1;
            }
        }
    }

    for param in &sig_generics.own_params {
        if !param.kind.is_ty_or_const() {
            mapping.insert(param.index, args_index as u32);
            args_index += 1;
        }
    }

    // If there are still unmapped lifetimes left and we are to map types and maybe self
    // then skip them, now it is the case when we generated more lifetimes then needed.
    // FIXME(fn_delegation): proper support for late bound lifetimes.
    while args_index < args.len() && args[args_index].as_region().is_some() {
        args_index += 1;
    }

    // If self after lifetimes insert mapping, relying that self is at 0 in sig parent
    if matches!(self_pos_kind, SelfPositionKind::AfterLifetimes { .. }) {
        mapping.insert(0, args_index as u32);
        args_index += 1;
    }

    if process_sig_parent_generics {
        for i in (sig_generics.has_self as usize)..sig_generics.parent_count {
            let param = sig_generics.param_at(i, tcx);
            if param.kind.is_ty_or_const() {
                mapping.insert(param.index, args_index as u32);
                args_index += 1;
            }
        }
    }

    for param in &sig_generics.own_params {
        if param.kind.is_ty_or_const() {
            mapping.insert(param.index, args_index as u32);
            args_index += 1;
        }
    }

    mapping
}

fn get_delegation_parent_args_count_without_self<'tcx>(
    tcx: TyCtxt<'tcx>,
    delegation_id: LocalDefId,
    sig_id: DefId,
) -> usize {
    let delegation_parent_args_count = tcx.generics_of(delegation_id).parent_count;

    match get_caller_and_callee_kind(tcx, delegation_id, sig_id) {
        (FnKind::Free, FnKind::Free)
        | (FnKind::Free, FnKind::AssocTrait)
        | (FnKind::AssocTraitImpl, FnKind::AssocTrait) => 0,

        (FnKind::AssocInherentImpl, FnKind::Free)
        | (FnKind::AssocInherentImpl, FnKind::AssocTrait) => {
            delegation_parent_args_count /* No Self in AssocInherentImpl */
        }

        (FnKind::AssocTrait, FnKind::Free) | (FnKind::AssocTrait, FnKind::AssocTrait) => {
            delegation_parent_args_count - 1 /* Without Self */
        }

        unsupported_caller_callee_kinds!() => unreachable!(),
    }
}

fn get_caller_and_callee_kind<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
    sig_id: DefId,
) -> (FnKind, FnKind) {
    (fn_kind(tcx, def_id.into()), fn_kind(tcx, sig_id))
}

fn get_parent_and_inheritance_kind<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
    sig_id: DefId,
) -> (Option<DefId>, InheritanceKind) {
    match get_caller_and_callee_kind(tcx, def_id, sig_id) {
        (FnKind::Free, FnKind::AssocTrait) => (
            None,
            InheritanceKind::WithParent(match should_propagate_self_ty(tcx, def_id) {
                PropagateSelfTy::No => SelfPredsFilterKind::None,
                PropagateSelfTy::Yes => SelfPredsFilterKind::All,
            }),
        ),

        (FnKind::Free, FnKind::Free) => {
            (None, InheritanceKind::WithParent(SelfPredsFilterKind::None))
        }

        (FnKind::AssocTraitImpl, FnKind::AssocTrait) => {
            (Some(tcx.parent(def_id.into())), InheritanceKind::Own)
        }

        (FnKind::AssocInherentImpl, FnKind::AssocTrait)
        | (FnKind::AssocTrait, FnKind::AssocTrait)
        | (FnKind::AssocInherentImpl, FnKind::Free)
        | (FnKind::AssocTrait, FnKind::Free) => (
            Some(tcx.parent(def_id.into())),
            InheritanceKind::WithParent(SelfPredsFilterKind::OnlyExplicitTrait),
        ),

        unsupported_caller_callee_kinds!() => unreachable!(),
    }
}

pub(crate) fn get_delegation_self_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    delegation_id: LocalDefId,
) -> Option<Ty<'tcx>> {
    let sig_id = tcx.hir_opt_delegation_sig_id(delegation_id).expect("Delegation must have sig_id");
    let (caller_kind, callee_kind) = get_caller_and_callee_kind(tcx, delegation_id, sig_id);

    match (caller_kind, callee_kind) {
        (FnKind::Free, FnKind::Free) => None,

        (FnKind::Free, FnKind::AssocTrait)
        | (FnKind::AssocInherentImpl, FnKind::Free)
        | (FnKind::AssocTrait, FnKind::Free)
        | (FnKind::AssocTrait, FnKind::AssocTrait) => {
            match create_self_position_kind(tcx, delegation_id, sig_id) {
                SelfPositionKind::None => None,
                SelfPositionKind::AfterLifetimes(propagate_self_ty) => match propagate_self_ty {
                    PropagateSelfTy::Yes => Some(get_delegation_self_ty_or_err(tcx, delegation_id)),
                    PropagateSelfTy::No => ty::GenericArgs::identity_for_item(tcx, delegation_id)
                        .iter()
                        .skip_while(|a| a.as_region().is_some())
                        .next()
                        .map(|a| a.as_type())
                        .flatten(),
                },
                SelfPositionKind::Zero => ty::GenericArgs::identity_for_item(tcx, delegation_id)
                    .first()
                    .map(|a| a.as_type())
                    .flatten(),
            }
        }

        (FnKind::AssocTraitImpl, FnKind::AssocTrait) => {
            create_trait_impl_to_trait_parent_args(tcx, delegation_id)
                .first()
                .map(|a| a.as_type())
                .flatten()
        }

        (FnKind::AssocInherentImpl, FnKind::AssocTrait) => {
            Some(create_inherent_impl_to_trait_self_ty(tcx, delegation_id))
        }

        unsupported_caller_callee_kinds!() => unreachable!(),
    }
}

fn get_delegation_self_ty_or_err(tcx: TyCtxt<'_>, delegation_id: LocalDefId) -> Ty<'_> {
    get_delegation_generics_info(tcx, delegation_id)
        .self_ty_id
        .map(|id| {
            let ctx = ItemCtxt::new(tcx, delegation_id);
            ctx.lower_ty(tcx.hir_node(id).expect_ty())
        })
        .unwrap_or_else(|| {
            Ty::new_error_with_message(
                tcx,
                tcx.def_span(delegation_id),
                "The self type must be provided",
            )
        })
}

fn create_trait_impl_to_trait_parent_args<'tcx>(
    tcx: TyCtxt<'tcx>,
    delegation_id: LocalDefId,
) -> ty::GenericArgsRef<'tcx> {
    let parent = tcx.parent(delegation_id.into());
    tcx.impl_trait_header(parent).trait_ref.instantiate_identity().args
}

fn create_inherent_impl_to_trait_self_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    delegation_id: LocalDefId,
) -> Ty<'tcx> {
    let parent = tcx.parent(delegation_id.into());
    tcx.type_of(parent).instantiate_identity()
}

/// Creates generic arguments for further delegation signature and predicates instantiation.
/// Arguments can be user-specified (in this case they are in `parent_args` and `child_args`)
/// or propagated. User can specify either both `parent_args` and `child_args`, one of them or none,
/// that is why we firstly create generic arguments from generic params and then adjust them with
/// user-specified args.
///
/// The order of produced list is important, it must be of this pattern:
///
/// [SELF | maybe self in the beginning]
/// [PARENT | args of delegation parent]
/// [SIG PARENT LIFETIMES] <- `lifetimes_end_pos`
/// [SIG LIFETIMES]
/// [SELF | maybe self after lifetimes, when we reuse trait fn in free context]
/// [SIG PARENT TYPES/CONSTS]
/// [SIG TYPES/CONSTS]
fn create_generic_args<'tcx>(
    tcx: TyCtxt<'tcx>,
    sig_id: DefId,
    delegation_id: LocalDefId,
    mut parent_args: &[ty::GenericArg<'tcx>],
    child_args: &[ty::GenericArg<'tcx>],
) -> Vec<ty::GenericArg<'tcx>> {
    let (caller_kind, callee_kind) = get_caller_and_callee_kind(tcx, delegation_id, sig_id);

    let delegation_args = ty::GenericArgs::identity_for_item(tcx, delegation_id);
    let delegation_parent_args_count = tcx.generics_of(delegation_id).parent_count;

    let deleg_parent_args_without_self_count =
        get_delegation_parent_args_count_without_self(tcx, delegation_id, sig_id);

    let args = match (caller_kind, callee_kind) {
        (FnKind::Free, FnKind::Free)
        | (FnKind::Free, FnKind::AssocTrait)
        | (FnKind::AssocInherentImpl, FnKind::Free)
        | (FnKind::AssocTrait, FnKind::Free)
        | (FnKind::AssocTrait, FnKind::AssocTrait) => delegation_args,

        (FnKind::AssocTraitImpl, FnKind::AssocTrait) => {
            // Special case, as user specifies Trait args in impl trait header, we want to treat
            // them as parent args.
            parent_args = create_trait_impl_to_trait_parent_args(tcx, delegation_id);
            tcx.mk_args(&delegation_args[delegation_parent_args_count..])
        }

        (FnKind::AssocInherentImpl, FnKind::AssocTrait) => {
            let self_ty = create_inherent_impl_to_trait_self_ty(tcx, delegation_id);

            tcx.mk_args_from_iter(
                std::iter::once(ty::GenericArg::from(self_ty)).chain(delegation_args.iter()),
            )
        }

        unsupported_caller_callee_kinds!() => unreachable!(),
    };

    let mut new_args = vec![];

    let self_pos_kind = create_self_position_kind(tcx, delegation_id, sig_id);
    let mut lifetimes_end_pos;

    if !parent_args.is_empty() {
        let parent_args_lifetimes_count =
            parent_args.iter().filter(|a| a.as_region().is_some()).count();

        match self_pos_kind {
            SelfPositionKind::AfterLifetimes { .. } => {
                new_args.extend(parent_args.iter().skip(1).take(parent_args_lifetimes_count));

                lifetimes_end_pos = parent_args_lifetimes_count;

                new_args.push(parent_args[0]);

                new_args.extend(parent_args.iter().skip(1 + parent_args_lifetimes_count));
            }
            SelfPositionKind::Zero => {
                lifetimes_end_pos = 1 /* Self */ + parent_args_lifetimes_count;
                new_args.extend_from_slice(parent_args);

                for i in 0..deleg_parent_args_without_self_count {
                    new_args.insert(1 + i, args[1 + i]);
                }

                lifetimes_end_pos += deleg_parent_args_without_self_count;
            }
            // If we have parent args then we obtained them from trait, then self must be somewhere
            SelfPositionKind::None => unreachable!(),
        };
    } else {
        let self_impact = matches!(self_pos_kind, SelfPositionKind::Zero) as usize;

        lifetimes_end_pos = self_impact
            + deleg_parent_args_without_self_count
            + args
                .iter()
                .skip(self_impact)
                .skip(deleg_parent_args_without_self_count)
                .filter(|a| a.as_region().is_some())
                .count();

        new_args.extend_from_slice(args);

        // Parent args are empty, then if we should propagate self ty (meaning Self generic arg
        // was not generated) then we should insert it, as it won't be in `args`
        if matches!(self_pos_kind, SelfPositionKind::AfterLifetimes(PropagateSelfTy::Yes)) {
            new_args.insert(
                lifetimes_end_pos,
                ty::GenericArg::from(get_delegation_self_ty_or_err(tcx, delegation_id)),
            );
        }
    }

    if !child_args.is_empty() {
        let child_lifetimes_count = child_args.iter().filter(|a| a.as_region().is_some()).count();

        for i in 0..child_lifetimes_count {
            new_args.insert(lifetimes_end_pos + i, child_args[i]);
        }

        new_args.extend_from_slice(&child_args[child_lifetimes_count..]);
    } else {
        if !parent_args.is_empty() {
            let child_args = &delegation_args[delegation_parent_args_count..];

            let child_lifetimes_count =
                child_args.iter().take_while(|a| a.as_region().is_some()).count();

            for i in 0..child_lifetimes_count {
                new_args.insert(lifetimes_end_pos + i, child_args[i]);
            }

            // If self_ty is propagated it means that Self generic param was not generated.
            let skip_self =
                matches!(self_pos_kind, SelfPositionKind::AfterLifetimes(PropagateSelfTy::No));

            new_args.extend(child_args.iter().skip(child_lifetimes_count).skip(skip_self as usize));
        }
    }

    new_args
}

pub(crate) fn inherit_predicates_for_delegation_item<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
    sig_id: DefId,
    parent_args: &'tcx [ty::GenericArg<'tcx>],
    child_args: &'tcx [ty::GenericArg<'tcx>],
) -> ty::GenericPredicates<'tcx> {
    struct PredicatesCollector<'tcx> {
        tcx: TyCtxt<'tcx>,
        preds: Vec<(ty::Clause<'tcx>, Span)>,
        args: Vec<ty::GenericArg<'tcx>>,
        folder: ParamIndexRemapper<'tcx>,
        filter_out_self_preds: bool,
    }

    impl<'tcx> PredicatesCollector<'tcx> {
        fn new(
            tcx: TyCtxt<'tcx>,
            args: Vec<ty::GenericArg<'tcx>>,
            folder: ParamIndexRemapper<'tcx>,
            filter_out_self_preds: bool,
        ) -> PredicatesCollector<'tcx> {
            PredicatesCollector { tcx, preds: vec![], args, folder, filter_out_self_preds }
        }

        fn with_own_preds(
            mut self,
            f: impl Fn(DefId) -> ty::GenericPredicates<'tcx>,
            def_id: DefId,
        ) -> Self {
            let preds = f(def_id);
            let mut new_predicates = vec![];

            for pred in preds.predicates {
                if self.filter_out_self_preds
                    && let Some(trait_pred) = pred.0.as_trait_clause()
                    // Rely that Self has zero index.
                    && trait_pred.self_ty().skip_binder().is_param(0)
                {
                    continue;
                }

                new_predicates.push((pred.0.fold_with(&mut self.folder), pred.1));
            }

            let preds = GenericPredicates {
                parent: preds.parent.clone(),
                predicates: self.tcx.arena.alloc_slice(new_predicates.as_slice()),
            };

            let preds = EarlyBinder::bind(preds.predicates)
                .iter_instantiated_copied(self.tcx, self.args.as_slice());

            self.preds.extend(preds);

            self
        }

        fn with_preds(
            mut self,
            f: impl Fn(DefId) -> ty::GenericPredicates<'tcx> + Copy,
            def_id: DefId,
        ) -> Self {
            let preds = f(def_id);
            if let Some(parent_def_id) = preds.parent {
                self = self.with_own_preds(f, parent_def_id);
            }

            self.with_own_preds(f, def_id)
        }
    }

    let (folder, args) = create_folder_and_args(tcx, def_id, sig_id, parent_args, child_args);
    let (parent, inh_kind) = get_parent_and_inheritance_kind(tcx, def_id, sig_id);

    let collector = PredicatesCollector::new(tcx, args, folder, inh_kind.filter_all_self_preds());

    // `explicit_predicates_of` is used here to avoid copying `Self: Trait` predicate.
    // Note: `predicates_of` query can also add inferred outlives predicates, but that
    // is not the case here as `sig_id` is either a trait or a function.
    let preds = match inh_kind {
        InheritanceKind::WithParent(SelfPredsFilterKind::OnlyExplicitTrait) => {
            collector.with_preds(|def_id| tcx.explicit_predicates_of(def_id), sig_id)
        }

        InheritanceKind::WithParent(SelfPredsFilterKind::None | SelfPredsFilterKind::All) => {
            collector.with_preds(|def_id| tcx.predicates_of(def_id), sig_id)
        }

        InheritanceKind::Own => {
            collector.with_own_preds(|def_id| tcx.predicates_of(def_id), sig_id)
        }
    }
    .preds;

    ty::GenericPredicates { parent, predicates: tcx.arena.alloc_from_iter(preds) }
}

fn create_folder_and_args<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
    sig_id: DefId,
    parent_args: &'tcx [ty::GenericArg<'tcx>],
    child_args: &'tcx [ty::GenericArg<'tcx>],
) -> (ParamIndexRemapper<'tcx>, Vec<ty::GenericArg<'tcx>>) {
    let args = create_generic_args(tcx, sig_id, def_id, parent_args, child_args);
    let remap_table = create_mapping(tcx, sig_id, def_id, &args);

    (ParamIndexRemapper { tcx, remap_table }, args)
}

fn check_constraints<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
    sig_id: DefId,
) -> Result<(), ErrorGuaranteed> {
    let mut ret = Ok(());

    let mut emit = |descr| {
        ret = Err(tcx.dcx().emit_err(crate::errors::UnsupportedDelegation {
            span: tcx.def_span(def_id),
            descr,
            callee_span: tcx.def_span(sig_id),
        }));
    };

    if tcx.fn_sig(sig_id).skip_binder().skip_binder().c_variadic {
        // See issue #127443 for explanation.
        emit("delegation to C-variadic functions is not allowed");
    }

    ret
}

pub(crate) fn inherit_sig_for_delegation_item<'tcx>(
    tcx: TyCtxt<'tcx>,
    input: (LocalDefId, &'tcx [ty::GenericArg<'tcx>], &'tcx [ty::GenericArg<'tcx>]),
) -> &'tcx [Ty<'tcx>] {
    let (def_id, parent_args, child_args) = input;

    let sig_id = tcx.hir_opt_delegation_sig_id(def_id).expect("Delegation must have sig_id");
    let caller_sig = tcx.fn_sig(sig_id);

    if let Err(err) = check_constraints(tcx, def_id, sig_id) {
        let sig_len = caller_sig.instantiate_identity().skip_binder().inputs().len() + 1;
        let err_type = Ty::new_error(tcx, err);
        return tcx.arena.alloc_from_iter((0..sig_len).map(|_| err_type));
    }

    let (mut folder, args) = create_folder_and_args(tcx, def_id, sig_id, parent_args, child_args);
    let caller_sig = EarlyBinder::bind(caller_sig.skip_binder().fold_with(&mut folder));

    let sig = caller_sig.instantiate(tcx, args.as_slice()).skip_binder();
    let sig_iter = sig.inputs().iter().cloned().chain(std::iter::once(sig.output()));
    tcx.arena.alloc_from_iter(sig_iter)
}

pub(crate) fn get_delegation_generics_info(
    tcx: TyCtxt<'_>,
    delegation_id: LocalDefId,
) -> &DelegationGenerics {
    tcx.hir_node(tcx.local_def_id_to_hir_id(delegation_id))
        .fn_sig()
        .expect("Processing delegation")
        .decl
        .opt_delegation_generics_info()
        .expect("Processing delegation")
}
