use radroots_sql_core::utils;
use serde::Deserialize;
use serde_json::Value;

use crate::{ReplicaSql, SqlError, SqlExecutor};

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ReplicaTradeProductSummaryRow {
    pub id: String,
    pub key: String,
    pub category: String,
    pub title: String,
    pub summary: String,
    pub qty_amt: f64,
    pub qty_amt_exact: Option<String>,
    pub qty_unit: String,
    pub qty_label: Option<String>,
    pub qty_avail: Option<i64>,
    pub price_amt: f64,
    pub price_amt_exact: Option<String>,
    pub price_currency: String,
    pub price_qty_amt: f64,
    pub price_qty_amt_exact: Option<String>,
    pub price_qty_unit: String,
    pub listing_addr: Option<String>,
    pub primary_bin_id: Option<String>,
    pub verified_primary_bin_id: Option<String>,
    pub notes: Option<String>,
    pub location_primary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ReplicaFarmDTagRow {
    d_tag: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ReplicaEventFreshnessRow {
    last_created_at: Option<i64>,
}

impl<E: SqlExecutor> ReplicaSql<E> {
    pub fn trade_product_lookup(
        &self,
        lookup: &str,
    ) -> Result<Vec<ReplicaTradeProductSummaryRow>, SqlError> {
        let sql = "SELECT tp.id, tp.key, tp.category, tp.title, tp.summary, tp.qty_amt, tp.qty_amt_exact, tp.qty_unit, tp.qty_label, tp.qty_avail, tp.price_amt, tp.price_amt_exact, tp.price_currency, tp.price_qty_amt, tp.price_qty_amt_exact, tp.price_qty_unit, tp.listing_addr, tp.primary_bin_id, tp.verified_primary_bin_id, tp.notes, loc.location_primary \
             FROM trade_product tp \
             LEFT JOIN (\
                 SELECT tpl.tb_tp AS trade_product_id, MIN(COALESCE(gl.label, gl.gc_name, gl.gc_admin1_name, gl.gc_country_name, gl.d_tag)) AS location_primary \
                 FROM trade_product_location tpl \
                 JOIN gcs_location gl ON gl.id = tpl.tb_gl \
                 GROUP BY tpl.tb_tp\
             ) loc ON loc.trade_product_id = tp.id \
             WHERE tp.id = ? OR tp.key = ? \
             ORDER BY lower(tp.title) ASC, tp.id ASC;";
        let params_json = utils::to_params_json(vec![
            Value::from(lookup.to_owned()),
            Value::from(lookup.to_owned()),
        ])?;
        let json = self.executor().query_raw(sql, &params_json)?;
        serde_json::from_str(&json).map_err(SqlError::from)
    }

    pub fn trade_product_search(
        &self,
        query_terms: &[String],
    ) -> Result<Vec<ReplicaTradeProductSummaryRow>, SqlError> {
        if query_terms.is_empty() {
            return Ok(Vec::new());
        }

        let mut where_clauses = Vec::with_capacity(query_terms.len());
        let mut bind_values = Vec::<Value>::with_capacity(query_terms.len() * 5);
        for term in query_terms {
            let pattern = format!("%{}%", term.to_lowercase());
            where_clauses.push(
                "(lower(tp.title) LIKE ? OR lower(tp.summary) LIKE ? OR lower(tp.category) LIKE ? OR lower(tp.key) LIKE ? OR lower(COALESCE(tp.notes, '')) LIKE ?)"
                    .to_owned(),
            );
            for _ in 0..5 {
                bind_values.push(Value::from(pattern.clone()));
            }
        }

        let sql = format!(
            "SELECT tp.id, tp.key, tp.category, tp.title, tp.summary, tp.qty_amt, tp.qty_amt_exact, tp.qty_unit, tp.qty_label, tp.qty_avail, tp.price_amt, tp.price_amt_exact, tp.price_currency, tp.price_qty_amt, tp.price_qty_amt_exact, tp.price_qty_unit, tp.listing_addr, tp.primary_bin_id, tp.verified_primary_bin_id, tp.notes, loc.location_primary \
             FROM trade_product tp \
             LEFT JOIN (\
                 SELECT tpl.tb_tp AS trade_product_id, MIN(COALESCE(gl.label, gl.gc_name, gl.gc_admin1_name, gl.gc_country_name, gl.d_tag)) AS location_primary \
                 FROM trade_product_location tpl \
                 JOIN gcs_location gl ON gl.id = tpl.tb_gl \
                 GROUP BY tpl.tb_tp\
             ) loc ON loc.trade_product_id = tp.id \
             WHERE {} \
             ORDER BY lower(tp.title) ASC, tp.id ASC;",
            where_clauses.join(" AND ")
        );
        let params_json = utils::to_params_json(bind_values)?;
        let json = self.executor().query_raw(&sql, &params_json)?;
        serde_json::from_str(&json).map_err(SqlError::from)
    }

    pub fn farm_unique_d_tag_by_pubkey(
        &self,
        seller_pubkey: &str,
    ) -> Result<Option<String>, SqlError> {
        let sql = "SELECT d_tag FROM farm WHERE pubkey = ? ORDER BY d_tag ASC;";
        let params_json = utils::to_params_json(vec![Value::from(seller_pubkey.to_owned())])?;
        let json = self.executor().query_raw(sql, &params_json)?;
        let rows: Vec<ReplicaFarmDTagRow> = serde_json::from_str(&json).map_err(SqlError::from)?;
        if rows.len() == 1 {
            Ok(Some(rows[0].d_tag.clone()))
        } else {
            Ok(None)
        }
    }

    pub fn nostr_event_last_created_at(&self) -> Result<Option<u64>, SqlError> {
        let json = self.executor().query_raw(
            "SELECT MAX(last_created_at) AS last_created_at FROM nostr_event_head WHERE last_created_at IS NOT NULL",
            "[]",
        )?;
        let rows: Vec<ReplicaEventFreshnessRow> =
            serde_json::from_str(&json).map_err(SqlError::from)?;
        Ok(rows
            .into_iter()
            .next()
            .and_then(|row| row.last_created_at)
            .and_then(|value| u64::try_from(value).ok()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_sql_core::ExecOutcome;

    struct QueryExecutor {
        farm_rows: &'static str,
        product_rows: &'static str,
    }

    impl SqlExecutor for QueryExecutor {
        fn exec(&self, _sql: &str, _params_json: &str) -> Result<ExecOutcome, SqlError> {
            Ok(ExecOutcome {
                changes: 0,
                last_insert_id: 0,
            })
        }

        fn query_raw(&self, sql: &str, _params_json: &str) -> Result<String, SqlError> {
            if sql.contains("FROM farm") {
                Ok(self.farm_rows.to_string())
            } else {
                Ok(self.product_rows.to_string())
            }
        }

        fn begin(&self) -> Result<(), SqlError> {
            Ok(())
        }

        fn commit(&self) -> Result<(), SqlError> {
            Ok(())
        }

        fn rollback(&self) -> Result<(), SqlError> {
            Ok(())
        }
    }

    fn product_rows() -> &'static str {
        r#"[{
            "id":"listing-1",
            "key":"coffee",
            "category":"produce",
            "title":"Coffee",
            "summary":"Washed coffee",
            "qty_amt":1.0,
            "qty_amt_exact":"1",
            "qty_unit":"kg",
            "qty_label":null,
            "qty_avail":10,
            "price_amt":12.0,
            "price_amt_exact":"12",
            "price_currency":"USD",
            "price_qty_amt":1.0,
            "price_qty_amt_exact":"1",
            "price_qty_unit":"kg",
            "listing_addr":"30402:pubkey:AAAAAAAAAAAAAAAAAAAAAA",
            "primary_bin_id":"bin-1",
            "verified_primary_bin_id":"bin-1",
            "notes":null,
            "location_primary":"Farm"
        }]"#
    }

    #[test]
    fn trade_product_queries_and_unique_farm_lookup_cover_empty_and_multiple_rows() {
        let db = ReplicaSql::new(QueryExecutor {
            farm_rows: r#"[{"d_tag":"farm-a"},{"d_tag":"farm-b"}]"#,
            product_rows: product_rows(),
        });

        assert_eq!(
            db.trade_product_search(&[]).expect("empty search"),
            Vec::new()
        );
        let lookup = db.trade_product_lookup("coffee").expect("lookup");
        assert_eq!(lookup[0].key, "coffee");
        assert_eq!(
            db.farm_unique_d_tag_by_pubkey("seller")
                .expect("farm lookup"),
            None
        );

        let unique_db = ReplicaSql::new(QueryExecutor {
            farm_rows: r#"[{"d_tag":"farm-a"}]"#,
            product_rows: product_rows(),
        });
        assert_eq!(
            unique_db
                .farm_unique_d_tag_by_pubkey("seller")
                .expect("farm lookup"),
            Some("farm-a".to_string())
        );
    }
}
