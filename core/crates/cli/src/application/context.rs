use std::sync::OnceLock;

use super::ports::{default_cli_ports, CliPorts};
use crate::*;

#[derive(Clone, Copy)]
pub(crate) struct CliAppContext<'a> {
    pub(crate) ports: CliPorts<'a>,
    pub(crate) runtime: &'a ReadOnlyRuntime,
    pub(crate) action_vm: &'a ReadOnlyActionVm,
    pub(crate) policy_kernel: &'a PolicyKernel,
}

pub(crate) fn default_app_context() -> CliAppContext<'static> {
    CliAppContext {
        ports: default_cli_ports(),
        runtime: default_runtime(),
        action_vm: default_action_vm(),
        policy_kernel: default_policy_kernel(),
    }
}

fn default_runtime() -> &'static ReadOnlyRuntime {
    static RUNTIME: OnceLock<ReadOnlyRuntime> = OnceLock::new();
    RUNTIME.get_or_init(ReadOnlyRuntime::default)
}

fn default_action_vm() -> &'static ReadOnlyActionVm {
    static ACTION_VM: OnceLock<ReadOnlyActionVm> = OnceLock::new();
    ACTION_VM.get_or_init(ReadOnlyActionVm::default)
}

fn default_policy_kernel() -> &'static PolicyKernel {
    static POLICY_KERNEL: OnceLock<PolicyKernel> = OnceLock::new();
    POLICY_KERNEL.get_or_init(|| PolicyKernel)
}
