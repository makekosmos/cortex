pub fn camel(k: &str) -> String {
    let mut s = String::new();
    let mut up = false;
    for c in k.chars() {
        if c == '_' {
            up = true
        } else if up {
            s.extend(c.to_uppercase());
            up = false
        } else {
            s.push(c)
        }
    }
    s
}
pub fn validate(obj: &crate::types::ArkObject) -> Result<(), CompatFailure> {
    let regs =
        crate::canonical_types::definitions::canonical_type_registrations().map_err(|_| {
            CompatFailure::DataLossRisk {
                pointer: "/".into(),
            }
        })?;
    let r = regs
        .into_iter()
        .find(|x| x.type_id == obj.type_id && x.version == obj.type_version)
        .ok_or_else(|| invalid("/type_id"))?;
    crate::canonical_types::validation::validate_canonical(&r, &obj.props_json, &obj.content_json)
        .map_err(|e| match e.code {
            crate::canonical_types::validation::CanonicalValidationCode::InvalidField => {
                invalid(&e.pointer)
            }
            crate::canonical_types::validation::CanonicalValidationCode::InvariantViolation => {
                CompatFailure::DataLossRisk { pointer: e.pointer }
            }
        })
}
