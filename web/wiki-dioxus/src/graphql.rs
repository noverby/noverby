use cynic::QueryBuilder;
use serde::{Deserialize, Serialize};

use crate::nhost::graphql_url;

mod schema {
    cynic::use_schema!("graphql/schema.graphql");
}

// Custom scalar types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub struct Uuid(pub String);
cynic::impl_scalar!(Uuid, schema::uuid);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timestamptz(pub String);
cynic::impl_scalar!(Timestamptz, schema::timestamptz);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Jsonb(pub serde_json::Value);
cynic::impl_scalar!(Jsonb, schema::jsonb);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bigint(pub String);
cynic::impl_scalar!(Bigint, schema::bigint);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bytea(pub String);
cynic::impl_scalar!(Bytea, schema::bytea);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Citext(pub String);
cynic::impl_scalar!(Citext, schema::citext);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text(pub String);
cynic::impl_scalar!(Text, schema::_text);

// --- Query: Fetch a single node by ID ---

#[derive(cynic::QueryVariables, Debug)]
pub struct NodeByIdVariables {
    pub id: Uuid,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(
    schema_path = "graphql/schema.graphql",
    graphql_type = "query_root",
    variables = "NodeByIdVariables"
)]
pub struct NodeByIdQuery {
    #[arguments(id: $id)]
    pub node: Option<NodeFields>,
}

// --- Query: Fetch nodes with a where filter ---

#[derive(cynic::QueryVariables, Debug)]
pub struct NodesWhereVariables {
    pub where_clause: NodesBoolExp,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(
    schema_path = "graphql/schema.graphql",
    graphql_type = "query_root",
    variables = "NodesWhereVariables"
)]
pub struct NodesWhereQuery {
    #[arguments(where: $where_clause)]
    pub nodes: Vec<NodeFields>,
}

// --- Node fields (basic — no children, no data) ---

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(schema_path = "graphql/schema.graphql", graphql_type = "nodes")]
pub struct NodeFields {
    pub id: Uuid,
    pub name: String,
    pub key: String,
    pub mime_id: Option<String>,
    pub parent_id: Option<Uuid>,
    pub context_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub mutable: bool,
    pub index: i32,
    pub mime: Option<MimeFields>,
}

// --- Node with children ---

#[derive(cynic::QueryVariables, Debug)]
pub struct NodeWithChildrenVariables {
    pub id: Uuid,
}

#[derive(cynic::QueryFragment, Debug, Clone)]
#[cynic(
    schema_path = "graphql/schema.graphql",
    graphql_type = "query_root",
    variables = "NodeWithChildrenVariables"
)]
pub struct NodeWithChildrenQuery {
    #[arguments(id: $id)]
    pub node: Option<NodeWithChildren>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(schema_path = "graphql/schema.graphql", graphql_type = "nodes")]
pub struct NodeWithChildren {
    pub id: Uuid,
    pub name: String,
    pub key: String,
    pub mime_id: Option<String>,
    pub parent_id: Option<Uuid>,
    pub context_id: Option<Uuid>,
    pub mutable: bool,
    pub index: i32,
    pub data: Option<Jsonb>,
    pub mime: Option<MimeFields>,
    pub parent: Option<Box<ParentNodeFields>>,
    pub children: Vec<ChildNodeFields>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(schema_path = "graphql/schema.graphql", graphql_type = "nodes")]
pub struct ParentNodeFields {
    pub id: Uuid,
    pub name: String,
    pub key: String,
    pub mime_id: Option<String>,
}

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(schema_path = "graphql/schema.graphql", graphql_type = "nodes")]
pub struct ChildNodeFields {
    pub id: Uuid,
    pub name: String,
    pub key: String,
    pub mime_id: Option<String>,
    pub mutable: bool,
    pub index: i32,
    pub mime: Option<MimeFields>,
}

// --- Mime type ---
// Schema: context: Boolean!, hidden: Boolean!, icon: String!, id: String!, unique: Boolean!

#[derive(cynic::QueryFragment, Debug, Clone, PartialEq)]
#[cynic(schema_path = "graphql/schema.graphql", graphql_type = "mimes")]
pub struct MimeFields {
    pub id: String,
    pub icon: String,
    pub hidden: bool,
    pub context: bool,
}

// --- Input types ---

#[derive(cynic::InputObject, Debug)]
#[cynic(
    schema_path = "graphql/schema.graphql",
    graphql_type = "nodes_bool_exp"
)]
pub struct NodesBoolExp {
    #[cynic(rename = "_and")]
    pub and: Option<Vec<NodesBoolExp>>,
    pub key: Option<StringComparisonExp>,
    pub parent_id: Option<UuidComparisonExp>,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(
    schema_path = "graphql/schema.graphql",
    graphql_type = "String_comparison_exp"
)]
pub struct StringComparisonExp {
    #[cynic(rename = "_eq")]
    pub eq: Option<String>,
    #[cynic(rename = "_ilike")]
    pub ilike: Option<String>,
    #[cynic(rename = "_is_null")]
    pub is_null: Option<bool>,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(
    schema_path = "graphql/schema.graphql",
    graphql_type = "uuid_comparison_exp"
)]
pub struct UuidComparisonExp {
    #[cynic(rename = "_eq")]
    pub eq: Option<Uuid>,
    #[cynic(rename = "_is_null")]
    pub is_null: Option<bool>,
}

// --- HTTP execution ---

pub async fn execute<Q, V>(
    access_token: Option<&str>,
    operation: cynic::Operation<Q, V>,
) -> Result<Q, String>
where
    Q: serde::de::DeserializeOwned + 'static,
    V: serde::Serialize,
{
    let client = reqwest::Client::new();
    let mut req = client.post(graphql_url());

    if let Some(token) = access_token {
        req = req.bearer_auth(token);
    }

    let resp = req
        .json(&operation)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: cynic::GraphQlResponse<Q> = resp.json().await.map_err(|e| e.to_string())?;

    if let Some(errors) = body.errors {
        let msgs: Vec<String> = errors.into_iter().map(|e| e.message).collect();
        return Err(msgs.join(", "));
    }

    body.data.ok_or_else(|| "No data returned".to_string())
}

// --- High-level query functions ---

pub async fn query_node_by_key(
    access_token: Option<&str>,
    key: &str,
    parent_id: Option<&str>,
) -> Result<Option<NodeFields>, String> {
    let where_clause = NodesBoolExp {
        and: Some(vec![
            NodesBoolExp {
                key: Some(StringComparisonExp {
                    eq: Some(key.to_string()),
                    ilike: None,
                    is_null: None,
                }),
                parent_id: None,
                and: None,
            },
            NodesBoolExp {
                key: None,
                parent_id: Some(match parent_id {
                    Some(id) => UuidComparisonExp {
                        eq: Some(Uuid(id.to_string())),
                        is_null: None,
                    },
                    None => UuidComparisonExp {
                        eq: None,
                        is_null: Some(true),
                    },
                }),
                and: None,
            },
        ]),
        key: None,
        parent_id: None,
    };

    let operation = NodesWhereQuery::build(NodesWhereVariables { where_clause });
    let result = execute(access_token, operation).await?;
    Ok(result.nodes.into_iter().next())
}

pub async fn query_node_by_id(
    access_token: Option<&str>,
    id: &str,
) -> Result<Option<NodeWithChildren>, String> {
    let operation = NodeWithChildrenQuery::build(NodeWithChildrenVariables {
        id: Uuid(id.to_string()),
    });
    let result = execute(access_token, operation).await?;
    Ok(result.node)
}

pub async fn resolve_path(
    access_token: Option<&str>,
    segments: &[String],
) -> Result<Option<NodeWithChildren>, String> {
    let mut parent_id: Option<String> = None;
    let mut last_node_id: Option<String> = None;

    for segment in segments {
        let found = query_node_by_key(access_token, segment, parent_id.as_deref()).await?;
        match found {
            Some(n) => {
                last_node_id = Some(n.id.0.clone());
                parent_id = Some(n.id.0);
            }
            None => return Ok(None),
        }
    }

    if let Some(id) = last_node_id {
        return query_node_by_id(access_token, &id).await;
    }

    Ok(None)
}
