//! Private service-controlled SQL admission policy.

#[derive(Clone, Copy)]
enum CreatePrefix {
    None,
    Create,
    CreateTemp,
    Other,
}

pub(crate) fn contains_forbidden_statement_control(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut cursor = 0;
    let mut statement_start = true;
    let mut create_prefix = CreatePrefix::None;
    let mut trigger_definition = false;
    let mut trigger_body = false;
    let mut trigger_case_depth = 0_u32;
    let mut trigger_end_seen = false;

    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if statement_start && bytes[cursor..].starts_with(&[0xef, 0xbb, 0xbf]) {
            cursor += 3;
            continue;
        }
        if byte.is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if byte == b'-' && bytes.get(cursor + 1) == Some(&b'-') {
            cursor += 2;
            while cursor < bytes.len() && !matches!(bytes[cursor], b'\n' | b'\r') {
                cursor += 1;
            }
            continue;
        }
        if byte == b'/' && bytes.get(cursor + 1) == Some(&b'*') {
            cursor += 2;
            while cursor + 1 < bytes.len() && !(bytes[cursor] == b'*' && bytes[cursor + 1] == b'/')
            {
                cursor += 1;
            }
            cursor = bytes.len().min(cursor + 2);
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`' | b'[') {
            let terminator = if byte == b'[' { b']' } else { byte };
            statement_start = false;
            create_prefix = CreatePrefix::Other;
            cursor += 1;
            while cursor < bytes.len() {
                if bytes[cursor] == terminator {
                    if bytes.get(cursor + 1) == Some(&terminator) {
                        cursor += 2;
                    } else {
                        cursor += 1;
                        break;
                    }
                } else {
                    cursor += 1;
                }
            }
            continue;
        }
        if byte == b';' {
            if trigger_definition && trigger_body && !trigger_end_seen {
                cursor += 1;
                continue;
            }
            statement_start = true;
            create_prefix = CreatePrefix::None;
            trigger_definition = false;
            trigger_body = false;
            trigger_case_depth = 0;
            trigger_end_seen = false;
            cursor += 1;
            continue;
        }
        if !is_identifier(byte) {
            statement_start = false;
            create_prefix = CreatePrefix::Other;
            cursor += 1;
            continue;
        }

        let start = cursor;
        cursor += 1;
        while cursor < bytes.len() && is_identifier(bytes[cursor]) {
            cursor += 1;
        }
        let token = &bytes[start..cursor];

        if trigger_definition {
            if !trigger_body && token.eq_ignore_ascii_case(b"begin") {
                trigger_body = true;
            } else if trigger_body && token.eq_ignore_ascii_case(b"case") {
                trigger_case_depth = trigger_case_depth.saturating_add(1);
            } else if trigger_body && token.eq_ignore_ascii_case(b"end") {
                if trigger_case_depth == 0 {
                    trigger_end_seen = true;
                } else {
                    trigger_case_depth -= 1;
                }
            }
            continue;
        }

        if statement_start {
            if is_forbidden(token) {
                return true;
            }
            statement_start = false;
            create_prefix = if token.eq_ignore_ascii_case(b"create") {
                CreatePrefix::Create
            } else {
                CreatePrefix::Other
            };
            continue;
        }

        create_prefix = match create_prefix {
            CreatePrefix::Create
                if token.eq_ignore_ascii_case(b"temp")
                    || token.eq_ignore_ascii_case(b"temporary") =>
            {
                CreatePrefix::CreateTemp
            }
            CreatePrefix::Create | CreatePrefix::CreateTemp
                if token.eq_ignore_ascii_case(b"trigger") =>
            {
                trigger_definition = true;
                CreatePrefix::None
            }
            CreatePrefix::Create | CreatePrefix::CreateTemp => CreatePrefix::Other,
            other => other,
        };
    }

    false
}

fn is_identifier(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_forbidden(token: &[u8]) -> bool {
    [
        b"pragma".as_slice(),
        b"attach",
        b"detach",
        b"begin",
        b"commit",
        b"end",
        b"rollback",
        b"savepoint",
        b"release",
    ]
    .into_iter()
    .any(|forbidden| token.eq_ignore_ascii_case(forbidden))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statement_control_inventory_is_closed_and_case_insensitive() {
        for forbidden in [
            "/* ignored before control */ PRAGMA trusted_schema = ON",
            "\u{feff}PRAGMA trusted_schema = ON",
            "ATTACH DATABASE 'x' AS extra",
            "detach database extra",
            " /* ignored */ BeGiN IMMEDIATE",
            "SELECT 1; -- ignored\n CoMmIt",
            "END TRANSACTION",
            "ROLLBACK TO escaped",
            "SAVEPOINT escaped",
            "RELEASE SAVEPOINT escaped",
            "CREATE TRIGGER audit_insert AFTER INSERT ON items BEGIN INSERT INTO audit_log (value) VALUES (NEW.value); END; /* after trigger */ COMMIT",
        ] {
            assert!(contains_forbidden_statement_control(forbidden));
        }
    }

    #[test]
    fn values_identifiers_comments_case_and_triggers_remain_available() {
        for allowed in [
            "SELECT 'pragma attach detach begin commit end rollback savepoint release'",
            "SELECT CASE WHEN value = 1 THEN 'commit' ELSE 'end' END FROM items",
            "SELECT 1 /* PRAGMA ATTACH COMMIT */",
            "CREATE TRIGGER audit_insert AFTER INSERT ON items BEGIN INSERT INTO audit_log (value) VALUES (CASE WHEN NEW.value = 1 THEN 'commit' ELSE 'end' END); END;",
            "SELECT pragmatic FROM items",
            "SELECT attachment FROM items",
            "SELECT detached FROM items",
            "SELECT beginner FROM items",
            "SELECT committed FROM items",
            "SELECT ending FROM items",
            "SELECT rolled_back FROM items",
            "SELECT savepoints FROM items",
            "SELECT released FROM items",
            "SELECT COUNT(*) FROM host_probe",
        ] {
            assert!(!contains_forbidden_statement_control(allowed));
        }
    }

    #[test]
    fn lexical_edges_remain_bounded_and_do_not_invent_statement_control() {
        for allowed in [
            "",
            "-",
            "-- unterminated comment",
            "/",
            "/* unterminated comment",
            "/* interior * is not a terminator */ SELECT 1",
            "''",
            "'unterminated value",
            "'escaped''quote'",
            "[]",
            "[unterminated identifier",
            "SELECT 1;",
            "CREATE TABLE items (value TEXT)",
            "CREATE TEMP TABLE items (value TEXT)",
            "CREATE TEMPORARY TRIGGER audit AFTER INSERT ON items BEGIN SELECT 1; END;",
            "CREATE TRIGGER incomplete; SELECT 1",
        ] {
            assert!(
                !contains_forbidden_statement_control(allowed),
                "lexical edge was misclassified: {allowed:?}"
            );
        }
    }
}
