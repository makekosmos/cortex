//! `INetFwPolicy2` (Windows Firewall COM API) implementation of
//! [`FirewallPolicy`]. Runs inside the privileged service as LocalSystem.
//!
//! No `netsh`/PowerShell: rule reads and writes go through the COM objects
//! only. The rules collection holds no exclusivity concern for us — the rule
//! name is stable and owned by this product.

#![cfg(windows)]

use windows::core::BSTR;
use windows::Win32::Foundation::VARIANT_BOOL;
use windows::Win32::NetworkManagement::WindowsFirewall::{
    INetFwPolicy2, INetFwRule, INetFwRules, NetFwPolicy2, NetFwRule, NET_FW_ACTION_ALLOW,
    NET_FW_ACTION_BLOCK, NET_FW_RULE_DIR_IN,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};

use super::{FirewallPolicy, ObservedRule, RuleAction, RuleSpec};

fn bstr(s: &str) -> BSTR {
    BSTR::from(s)
}

/// Live `INetFwPolicy2` handle.
pub struct NetFwPolicy2 {
    rules: INetFwRules,
}

impl NetFwPolicy2 {
    pub fn open() -> Result<Self, String> {
        unsafe {
            // Already-initialized threads return S_FALSE (Ok); a mismatched
            // model surfaces as RPC_E_CHANGED_MODE on the next call anyway.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let policy: INetFwPolicy2 =
                CoCreateInstance(&NetFwPolicy2, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            let rules = policy.Rules().map_err(|e| e.to_string())?;
            Ok(Self { rules })
        }
    }

    /// Instantiate a fresh `INetFwRule` COM object and fill it from `spec`.
    fn build_rule(&self, spec: &RuleSpec) -> Result<INetFwRule, String> {
        unsafe {
            let rule: INetFwRule =
                CoCreateInstance(&NetFwRule, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            fill_rule(&rule, spec)?;
            Ok(rule)
        }
    }
}

/// Set every managed field on an existing or new rule object.
unsafe fn fill_rule(rule: &INetFwRule, spec: &RuleSpec) -> Result<(), String> {
    unsafe {
        rule.SetName(&bstr(spec.name)).map_err(|e| e.to_string())?;
        rule.SetGrouping(&bstr(spec.grouping))
            .map_err(|e| e.to_string())?;
        rule.SetDescription(&bstr(spec.description))
            .map_err(|e| e.to_string())?;
        rule.SetApplicationName(&bstr(&spec.program))
            .map_err(|e| e.to_string())?;
        rule.SetProtocol(spec.protocol).map_err(|e| e.to_string())?;
        rule.SetProfiles(spec.profiles).map_err(|e| e.to_string())?;
        rule.SetDirection(NET_FW_RULE_DIR_IN)
            .map_err(|e| e.to_string())?;
        rule.SetAction(match spec.action {
            RuleAction::Allow => NET_FW_ACTION_ALLOW,
            RuleAction::Block => NET_FW_ACTION_BLOCK,
        })
        .map_err(|e| e.to_string())?;
        rule.SetInterfaceTypes(&bstr("All"))
            .map_err(|e| e.to_string())?;
        rule.SetLocalPorts(&bstr("*")).map_err(|e| e.to_string())?;
        rule.SetRemoteAddresses(&bstr("*"))
            .map_err(|e| e.to_string())?;
        rule.SetEnabled(VARIANT_BOOL::from(true))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_rule(rule: &INetFwRule) -> Result<ObservedRule, String> {
    unsafe {
        Ok(ObservedRule {
            program: rule.ApplicationName().ok().map(|b| b.to_string()),
            enabled: rule.Enabled().map(|v| v.as_bool()).unwrap_or(false),
            protocol: rule.Protocol().unwrap_or_default(),
            profiles: rule.Profiles().unwrap_or_default(),
            direction_inbound: rule
                .Direction()
                .map(|d| d == NET_FW_RULE_DIR_IN)
                .unwrap_or(false),
            action: match rule.Action().unwrap_or(NET_FW_ACTION_BLOCK) {
                NET_FW_ACTION_ALLOW => RuleAction::Allow,
                _ => RuleAction::Block,
            },
        })
    }
}

impl FirewallPolicy for NetFwPolicy2 {
    fn rule(&self, name: &str) -> Result<Option<ObservedRule>, String> {
        unsafe {
            // `Item` returns an error HRESULT when no rule of that name
            // exists — that is the "None" path, not a failure.
            match self.rules.Item(&bstr(name)) {
                Ok(rule) => read_rule(&rule).map(Some),
                Err(_) => Ok(None),
            }
        }
    }

    fn put(&mut self, spec: &RuleSpec) -> Result<(), String> {
        unsafe {
            if let Ok(existing) = self.rules.Item(&bstr(spec.name)) {
                // Updating the fields in place keeps the rule's identity
                // (and any external bookkeeping) intact.
                fill_rule(&existing, spec)
            } else {
                let rule = self.build_rule(spec)?;
                self.rules.Add(&rule).map_err(|e| e.to_string())
            }
        }
    }

    fn remove(&mut self, name: &str) -> Result<(), String> {
        unsafe {
            // Removing an absent rule errors inside COM — treat as done.
            let _ = self.rules.Remove(&bstr(name));
        }
        Ok(())
    }
}
