use std::sync::OnceLock;

use super::ports::CliPorts;
use crate::{PolicyKernel, ReadOnlyActionVm, ReadOnlyRuntime};

#[derive(Clone, Copy)]
pub(crate) struct CliAppContext<'a> {
    pub(crate) ports: CliPorts<'a>,
    pub(crate) runtime: &'a ReadOnlyRuntime,
    pub(crate) action_vm: &'a ReadOnlyActionVm,
    pub(crate) policy_kernel: &'a PolicyKernel,
}

impl<'a> CliAppContext<'a> {
    pub(crate) const fn new(
        ports: CliPorts<'a>,
        runtime: &'a ReadOnlyRuntime,
        action_vm: &'a ReadOnlyActionVm,
        policy_kernel: &'a PolicyKernel,
    ) -> Self {
        Self {
            ports,
            runtime,
            action_vm,
            policy_kernel,
        }
    }
}

pub(crate) fn default_runtime() -> &'static ReadOnlyRuntime {
    static RUNTIME: OnceLock<ReadOnlyRuntime> = OnceLock::new();
    RUNTIME.get_or_init(ReadOnlyRuntime::default)
}

pub(crate) fn default_action_vm() -> &'static ReadOnlyActionVm {
    static ACTION_VM: OnceLock<ReadOnlyActionVm> = OnceLock::new();
    ACTION_VM.get_or_init(ReadOnlyActionVm::default)
}

pub(crate) fn default_policy_kernel() -> &'static PolicyKernel {
    static POLICY_KERNEL: OnceLock<PolicyKernel> = OnceLock::new();
    POLICY_KERNEL.get_or_init(|| PolicyKernel)
}
