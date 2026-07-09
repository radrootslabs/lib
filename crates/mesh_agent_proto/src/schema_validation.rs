use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    RADROOTS_MESH_AGENT_SCHEMA_ID, RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE,
    RadrootsMeshAgentProtoError,
};

struct RequiredField {
    owner: &'static str,
    name: &'static str,
    ordinal: u16,
    field_type: &'static str,
}

struct RequiredVariant {
    owner: &'static str,
    name: &'static str,
    ordinal: u16,
}

const REQUEST_FIELDS: &[RequiredField] = &[
    field("MeshAgentRequest", "requestId", 0, "Text"),
    field("MeshAgentRequest", "action", 1, "MeshAgentAction"),
    field("MeshAgentRequest", "frameCbor", 2, "Data"),
    field(
        "MeshAgentRequest",
        "statusRequest",
        3,
        "MeshAgentStatusRequest",
    ),
    field(
        "MeshAgentRequest",
        "publishRequest",
        4,
        "MeshAgentPublishRequest",
    ),
];

const ACTION_VARIANTS: &[RequiredVariant] = &[
    variant("MeshAgentAction", "validateFrame", 0),
    variant("MeshAgentAction", "stageDelivery", 1),
    variant("MeshAgentAction", "observeEventHead", 2),
    variant("MeshAgentAction", "status", 3),
    variant("MeshAgentAction", "publish", 4),
];

const RESPONSE_FIELDS: &[RequiredField] = &[
    field("MeshAgentResponse", "requestId", 0, "Text"),
    field("MeshAgentResponse", "status", 1, "MeshAgentResponseStatus"),
    field("MeshAgentResponse", "receipt", 2, "MeshAgentReceipt"),
    field("MeshAgentResponse", "errors", 3, "List(MeshAgentError)"),
    field(
        "MeshAgentResponse",
        "statusResponse",
        4,
        "MeshAgentStatusResponse",
    ),
    field(
        "MeshAgentResponse",
        "publishResponse",
        5,
        "MeshAgentPublishResponse",
    ),
];

const RESPONSE_STATUS_VARIANTS: &[RequiredVariant] = &[
    variant("MeshAgentResponseStatus", "accepted", 0),
    variant("MeshAgentResponseStatus", "deferred", 1),
    variant("MeshAgentResponseStatus", "rejected", 2),
];

const RECEIPT_FIELDS: &[RequiredField] = &[
    field("MeshAgentReceipt", "frameDigest", 0, "Text"),
    field("MeshAgentReceipt", "acceptedEventHeads", 1, "List(Text)"),
];

const STATUS_FIELDS: &[RequiredField] = &[
    field("MeshAgentStatusRequest", "includeTransports", 0, "Bool"),
    field(
        "MeshAgentStatusResponse",
        "readiness",
        0,
        "MeshAgentReadinessState",
    ),
    field(
        "MeshAgentStatusResponse",
        "implementationState",
        1,
        "MeshAgentImplementationState",
    ),
    field(
        "MeshAgentStatusResponse",
        "transports",
        2,
        "List(MeshAgentTransportStatus)",
    ),
    field(
        "MeshAgentTransportStatus",
        "transportKind",
        0,
        "MeshAgentTransportKind",
    ),
    field("MeshAgentTransportStatus", "profileId", 1, "Text"),
    field("MeshAgentTransportStatus", "endpointUri", 2, "Text"),
    field(
        "MeshAgentTransportStatus",
        "readiness",
        3,
        "MeshAgentReadinessState",
    ),
    field(
        "MeshAgentTransportStatus",
        "implementationState",
        4,
        "MeshAgentImplementationState",
    ),
    field("MeshAgentTransportStatus", "publishUsable", 5, "Bool"),
    field("MeshAgentTransportStatus", "fetchUsable", 6, "Bool"),
    field("MeshAgentTransportStatus", "redactedMessage", 7, "Text"),
];

const READINESS_VARIANTS: &[RequiredVariant] = &[
    variant("MeshAgentReadinessState", "ready", 0),
    variant("MeshAgentReadinessState", "disabled", 1),
    variant("MeshAgentReadinessState", "misconfigured", 2),
    variant("MeshAgentReadinessState", "previewUnavailable", 3),
];

const IMPLEMENTATION_VARIANTS: &[RequiredVariant] = &[
    variant("MeshAgentImplementationState", "previewNoop", 0),
    variant("MeshAgentImplementationState", "mock", 1),
    variant("MeshAgentImplementationState", "real", 2),
];

const TRANSPORT_KIND_VARIANTS: &[RequiredVariant] =
    &[variant("MeshAgentTransportKind", "reticulum", 0)];

const TRANSPORT_OUTCOME_VARIANTS: &[RequiredVariant] = &[
    variant("MeshAgentTransportOutcome", "accepted", 0),
    variant("MeshAgentTransportOutcome", "delivered", 1),
    variant("MeshAgentTransportOutcome", "forwarded", 2),
    variant("MeshAgentTransportOutcome", "storedByGateway", 3),
    variant("MeshAgentTransportOutcome", "deferredUntilImplemented", 4),
    variant("MeshAgentTransportOutcome", "rejected", 5),
    variant("MeshAgentTransportOutcome", "routeUnavailable", 6),
    variant("MeshAgentTransportOutcome", "timeout", 7),
    variant("MeshAgentTransportOutcome", "transportUnavailable", 8),
];

const PUBLISH_FIELDS: &[RequiredField] = &[
    field("MeshAgentPublishRequest", "publishRequestId", 0, "Text"),
    field("MeshAgentPublishRequest", "payloadCbor", 1, "Data"),
    field("MeshAgentPublishRequest", "eventId", 2, "Text"),
    field("MeshAgentPublishRequest", "targetFingerprint", 3, "Text"),
    field("MeshAgentPublishResponse", "publishRequestId", 0, "Text"),
    field(
        "MeshAgentPublishResponse",
        "status",
        1,
        "MeshAgentResponseStatus",
    ),
    field(
        "MeshAgentPublishResponse",
        "transportReceipts",
        2,
        "List(MeshAgentTransportReceipt)",
    ),
    field("MeshAgentPublishResponse", "eventId", 3, "Text"),
    field(
        "MeshAgentTransportReceipt",
        "transportKind",
        0,
        "MeshAgentTransportKind",
    ),
    field("MeshAgentTransportReceipt", "endpointUri", 1, "Text"),
    field(
        "MeshAgentTransportReceipt",
        "outcome",
        2,
        "MeshAgentTransportOutcome",
    ),
    field("MeshAgentTransportReceipt", "redactedMessage", 3, "Text"),
];

const ERROR_FIELDS: &[RequiredField] = &[
    field("MeshAgentError", "code", 0, "Text"),
    field("MeshAgentError", "message", 1, "Text"),
];

const fn field(
    owner: &'static str,
    name: &'static str,
    ordinal: u16,
    field_type: &'static str,
) -> RequiredField {
    RequiredField {
        owner,
        name,
        ordinal,
        field_type,
    }
}

const fn variant(owner: &'static str, name: &'static str, ordinal: u16) -> RequiredVariant {
    RequiredVariant {
        owner,
        name,
        ordinal,
    }
}

pub(crate) fn validate_schema_text(schema: &str) -> Result<(), RadrootsMeshAgentProtoError> {
    let parsed = parse_schema(schema)?;
    if parsed.schema_id.as_deref() != Some(RADROOTS_MESH_AGENT_SCHEMA_ID) {
        return Err(RadrootsMeshAgentProtoError::MissingSchemaId);
    }
    if parsed.namespace.as_deref() != Some(RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE) {
        return Err(RadrootsMeshAgentProtoError::MissingNamespace);
    }
    validate_fields(
        &parsed,
        REQUEST_FIELDS,
        RadrootsMeshAgentProtoError::MissingRequest,
    )?;
    validate_variants(
        &parsed,
        ACTION_VARIANTS,
        RadrootsMeshAgentProtoError::MissingAction,
    )?;
    validate_fields(
        &parsed,
        RESPONSE_FIELDS,
        RadrootsMeshAgentProtoError::MissingResponse,
    )?;
    validate_variants(
        &parsed,
        RESPONSE_STATUS_VARIANTS,
        RadrootsMeshAgentProtoError::MissingResponse,
    )?;
    validate_fields(
        &parsed,
        RECEIPT_FIELDS,
        RadrootsMeshAgentProtoError::MissingReceipt,
    )?;
    validate_fields(
        &parsed,
        STATUS_FIELDS,
        RadrootsMeshAgentProtoError::MissingStatusSurface,
    )?;
    validate_variants(
        &parsed,
        READINESS_VARIANTS,
        RadrootsMeshAgentProtoError::MissingStatusSurface,
    )?;
    validate_variants(
        &parsed,
        IMPLEMENTATION_VARIANTS,
        RadrootsMeshAgentProtoError::MissingStatusSurface,
    )?;
    validate_variants(
        &parsed,
        TRANSPORT_KIND_VARIANTS,
        RadrootsMeshAgentProtoError::MissingStatusSurface,
    )?;
    validate_fields(
        &parsed,
        PUBLISH_FIELDS,
        RadrootsMeshAgentProtoError::MissingPublishSurface,
    )?;
    validate_variants(
        &parsed,
        TRANSPORT_OUTCOME_VARIANTS,
        RadrootsMeshAgentProtoError::MissingPublishSurface,
    )?;
    validate_fields(
        &parsed,
        ERROR_FIELDS,
        RadrootsMeshAgentProtoError::MissingError,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SchemaToken {
    Ident(String),
    Number(String),
    StringLiteral(String),
    Symbol(char),
}

#[derive(Default)]
struct SchemaAst {
    schema_id: Option<String>,
    namespace: Option<String>,
    structs: BTreeMap<String, StructDecl>,
    enums: BTreeMap<String, EnumDecl>,
}

struct StructDecl {
    fields: BTreeMap<String, FieldDecl>,
}

struct FieldDecl {
    ordinal: u16,
    field_type: String,
}

struct EnumDecl {
    variants: BTreeMap<String, u16>,
}

struct SchemaParser {
    tokens: Vec<SchemaToken>,
    index: usize,
}

fn parse_schema(schema: &str) -> Result<SchemaAst, RadrootsMeshAgentProtoError> {
    let tokens = lex_schema(schema)?;
    let mut parser = SchemaParser { tokens, index: 0 };
    parser.parse()
}

fn lex_schema(schema: &str) -> Result<Vec<SchemaToken>, RadrootsMeshAgentProtoError> {
    let chars: Vec<char> = schema.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            index += 1;
            continue;
        }
        if ch == '#' {
            index = skip_line_comment(&chars, index + 1);
            continue;
        }
        if ch == '/' && chars.get(index + 1) == Some(&'/') {
            index = skip_line_comment(&chars, index + 2);
            continue;
        }
        if ch == '/' && chars.get(index + 1) == Some(&'*') {
            index = skip_block_comment(&chars, index + 2)?;
            continue;
        }
        if is_ident_start(ch) {
            let start = index;
            index += 1;
            while index < chars.len() && is_ident_continue(chars[index]) {
                index += 1;
            }
            tokens.push(SchemaToken::Ident(chars[start..index].iter().collect()));
            continue;
        }
        if ch.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < chars.len() && is_number_continue(chars[index]) {
                index += 1;
            }
            tokens.push(SchemaToken::Number(chars[start..index].iter().collect()));
            continue;
        }
        if ch == '"' {
            let (literal, next_index) = read_string_literal(&chars, index + 1)?;
            tokens.push(SchemaToken::StringLiteral(literal));
            index = next_index;
            continue;
        }
        if is_schema_symbol(ch) {
            tokens.push(SchemaToken::Symbol(ch));
            index += 1;
            continue;
        }
        return Err(RadrootsMeshAgentProtoError::InvalidSchema);
    }
    Ok(tokens)
}

fn skip_line_comment(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() && chars[index] != '\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(
    chars: &[char],
    mut index: usize,
) -> Result<usize, RadrootsMeshAgentProtoError> {
    while index + 1 < chars.len() {
        if chars[index] == '*' && chars[index + 1] == '/' {
            return Ok(index + 2);
        }
        index += 1;
    }
    Err(RadrootsMeshAgentProtoError::InvalidSchema)
}

fn read_string_literal(
    chars: &[char],
    mut index: usize,
) -> Result<(String, usize), RadrootsMeshAgentProtoError> {
    let mut literal = String::new();
    while index < chars.len() {
        match chars[index] {
            '"' => return Ok((literal, index + 1)),
            '\\' => {
                index += 1;
                if index >= chars.len() {
                    return Err(RadrootsMeshAgentProtoError::InvalidSchema);
                }
                literal.push(chars[index]);
            }
            ch => literal.push(ch),
        }
        index += 1;
    }
    Err(RadrootsMeshAgentProtoError::InvalidSchema)
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_number_continue(ch: char) -> bool {
    ch.is_ascii_hexdigit() || ch == 'x' || ch == 'X'
}

fn is_schema_symbol(ch: char) -> bool {
    matches!(
        ch,
        '@' | '{' | '}' | ';' | ':' | '(' | ')' | '.' | '$' | ',' | '='
    )
}

impl SchemaParser {
    fn parse(&mut self) -> Result<SchemaAst, RadrootsMeshAgentProtoError> {
        let mut ast = SchemaAst::default();
        while !self.is_done() {
            match self.peek() {
                Some(SchemaToken::Symbol('@')) => self.parse_schema_id(&mut ast)?,
                Some(SchemaToken::Symbol('$')) => self.parse_annotation(&mut ast)?,
                Some(SchemaToken::Ident(value)) if value == "using" => self.skip_statement()?,
                Some(SchemaToken::Ident(value)) if value == "struct" => {
                    let (name, decl) = self.parse_struct()?;
                    if ast.structs.insert(name, decl).is_some() {
                        return Err(RadrootsMeshAgentProtoError::InvalidSchema);
                    }
                }
                Some(SchemaToken::Ident(value)) if value == "enum" => {
                    let (name, decl) = self.parse_enum()?;
                    if ast.enums.insert(name, decl).is_some() {
                        return Err(RadrootsMeshAgentProtoError::InvalidSchema);
                    }
                }
                Some(SchemaToken::Symbol(';')) => {
                    self.index += 1;
                }
                _ => return Err(RadrootsMeshAgentProtoError::InvalidSchema),
            }
        }
        Ok(ast)
    }

    fn parse_schema_id(&mut self, ast: &mut SchemaAst) -> Result<(), RadrootsMeshAgentProtoError> {
        self.expect_symbol('@')?;
        let schema_id = self.expect_number()?;
        self.expect_symbol(';')?;
        if ast.schema_id.replace(schema_id).is_some() {
            return Err(RadrootsMeshAgentProtoError::InvalidSchema);
        }
        Ok(())
    }

    fn parse_annotation(&mut self, ast: &mut SchemaAst) -> Result<(), RadrootsMeshAgentProtoError> {
        self.expect_symbol('$')?;
        let is_namespace = self.consume_ident("Cxx")
            && self.consume_symbol('.')
            && self.consume_ident("namespace")
            && self.consume_symbol('(');
        if is_namespace {
            let namespace = self.expect_string_literal()?;
            self.expect_symbol(')')?;
            self.expect_symbol(';')?;
            if ast.namespace.replace(namespace).is_some() {
                return Err(RadrootsMeshAgentProtoError::InvalidSchema);
            }
            return Ok(());
        }
        self.skip_statement()
    }

    fn parse_struct(&mut self) -> Result<(String, StructDecl), RadrootsMeshAgentProtoError> {
        self.expect_ident_value("struct")?;
        let name = self.expect_ident()?;
        self.expect_symbol('{')?;
        let mut fields = BTreeMap::new();
        let mut ordinals = BTreeSet::new();
        while !self.consume_symbol('}') {
            let field_name = self.expect_ident()?;
            self.expect_symbol('@')?;
            let ordinal = parse_ordinal(&self.expect_number()?)?;
            self.expect_symbol(':')?;
            let field_type = self.parse_type()?;
            if !ordinals.insert(ordinal)
                || fields
                    .insert(
                        field_name,
                        FieldDecl {
                            ordinal,
                            field_type,
                        },
                    )
                    .is_some()
            {
                return Err(RadrootsMeshAgentProtoError::InvalidSchema);
            }
        }
        Ok((name, StructDecl { fields }))
    }

    fn parse_enum(&mut self) -> Result<(String, EnumDecl), RadrootsMeshAgentProtoError> {
        self.expect_ident_value("enum")?;
        let name = self.expect_ident()?;
        self.expect_symbol('{')?;
        let mut variants = BTreeMap::new();
        let mut ordinals = BTreeSet::new();
        while !self.consume_symbol('}') {
            let variant_name = self.expect_ident()?;
            self.expect_symbol('@')?;
            let ordinal = parse_ordinal(&self.expect_number()?)?;
            self.expect_symbol(';')?;
            if !ordinals.insert(ordinal) || variants.insert(variant_name, ordinal).is_some() {
                return Err(RadrootsMeshAgentProtoError::InvalidSchema);
            }
        }
        Ok((name, EnumDecl { variants }))
    }

    fn parse_type(&mut self) -> Result<String, RadrootsMeshAgentProtoError> {
        let mut field_type = String::new();
        while let Some(token) = self.peek() {
            if token == &SchemaToken::Symbol(';') {
                self.index += 1;
                if field_type.is_empty() {
                    return Err(RadrootsMeshAgentProtoError::InvalidSchema);
                }
                return Ok(field_type);
            }
            append_token_text(&mut field_type, token)?;
            self.index += 1;
        }
        Err(RadrootsMeshAgentProtoError::InvalidSchema)
    }

    fn skip_statement(&mut self) -> Result<(), RadrootsMeshAgentProtoError> {
        while !self.is_done() {
            if self.consume_symbol(';') {
                return Ok(());
            }
            self.index += 1;
        }
        Err(RadrootsMeshAgentProtoError::InvalidSchema)
    }

    fn peek(&self) -> Option<&SchemaToken> {
        self.tokens.get(self.index)
    }

    fn is_done(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn consume_symbol(&mut self, symbol: char) -> bool {
        if self.peek() == Some(&SchemaToken::Symbol(symbol)) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_ident(&mut self, expected: &str) -> bool {
        match self.peek() {
            Some(SchemaToken::Ident(value)) if value == expected => {
                self.index += 1;
                true
            }
            _ => false,
        }
    }

    fn expect_symbol(&mut self, expected: char) -> Result<(), RadrootsMeshAgentProtoError> {
        if self.consume_symbol(expected) {
            Ok(())
        } else {
            Err(RadrootsMeshAgentProtoError::InvalidSchema)
        }
    }

    fn expect_ident_value(&mut self, expected: &str) -> Result<(), RadrootsMeshAgentProtoError> {
        if self.consume_ident(expected) {
            Ok(())
        } else {
            Err(RadrootsMeshAgentProtoError::InvalidSchema)
        }
    }

    fn expect_ident(&mut self) -> Result<String, RadrootsMeshAgentProtoError> {
        match self.peek().cloned() {
            Some(SchemaToken::Ident(value)) => {
                self.index += 1;
                Ok(value)
            }
            _ => Err(RadrootsMeshAgentProtoError::InvalidSchema),
        }
    }

    fn expect_number(&mut self) -> Result<String, RadrootsMeshAgentProtoError> {
        match self.peek().cloned() {
            Some(SchemaToken::Number(value)) => {
                self.index += 1;
                Ok(value)
            }
            _ => Err(RadrootsMeshAgentProtoError::InvalidSchema),
        }
    }

    fn expect_string_literal(&mut self) -> Result<String, RadrootsMeshAgentProtoError> {
        match self.peek().cloned() {
            Some(SchemaToken::StringLiteral(value)) => {
                self.index += 1;
                Ok(value)
            }
            _ => Err(RadrootsMeshAgentProtoError::InvalidSchema),
        }
    }
}

fn append_token_text(
    output: &mut String,
    token: &SchemaToken,
) -> Result<(), RadrootsMeshAgentProtoError> {
    match token {
        SchemaToken::Ident(value) | SchemaToken::Number(value) => {
            output.push_str(value.as_str());
            Ok(())
        }
        SchemaToken::Symbol(symbol) if matches!(symbol, '(' | ')' | '.') => {
            output.push(*symbol);
            Ok(())
        }
        _ => Err(RadrootsMeshAgentProtoError::InvalidSchema),
    }
}

fn parse_ordinal(value: &str) -> Result<u16, RadrootsMeshAgentProtoError> {
    value
        .parse::<u16>()
        .map_err(|_| RadrootsMeshAgentProtoError::InvalidSchema)
}

fn validate_fields(
    ast: &SchemaAst,
    fields: &[RequiredField],
    error: RadrootsMeshAgentProtoError,
) -> Result<(), RadrootsMeshAgentProtoError> {
    for required in fields {
        let decl = ast
            .structs
            .get(required.owner)
            .ok_or_else(|| error.clone())?;
        let field_matches = decl.fields.get(required.name).is_some_and(|field| {
            field.ordinal == required.ordinal && field.field_type == required.field_type
        });
        if !field_matches {
            return Err(error.clone());
        }
    }
    Ok(())
}

fn validate_variants(
    ast: &SchemaAst,
    variants: &[RequiredVariant],
    error: RadrootsMeshAgentProtoError,
) -> Result<(), RadrootsMeshAgentProtoError> {
    for required in variants {
        let decl = ast.enums.get(required.owner).ok_or_else(|| error.clone())?;
        let variant_matches = decl
            .variants
            .get(required.name)
            .is_some_and(|ordinal| *ordinal == required.ordinal);
        if !variant_matches {
            return Err(error.clone());
        }
    }
    Ok(())
}
