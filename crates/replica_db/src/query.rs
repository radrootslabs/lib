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
    pub qty_amt: i64,
    pub qty_unit: String,
    pub qty_label: Option<String>,
    pub qty_avail: Option<i64>,
    pub price_amt: f64,
    pub price_currency: String,
    pub price_qty_amt: u32,
    pub price_qty_unit: String,
    pub listing_addr: Option<String>,
    pub primary_bin_id: Option<String>,
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
        let sql = "SELECT tp.id, tp.key, tp.category, tp.title, tp.summary, tp.qty_amt, tp.qty_unit, tp.qty_label, tp.qty_avail, tp.price_amt, tp.price_currency, tp.price_qty_amt, tp.price_qty_unit, tp.listing_addr, tp.primary_bin_id, tp.notes, loc.location_primary \
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
            "SELECT tp.id, tp.key, tp.category, tp.title, tp.summary, tp.qty_amt, tp.qty_unit, tp.qty_label, tp.qty_avail, tp.price_amt, tp.price_currency, tp.price_qty_amt, tp.price_qty_unit, tp.listing_addr, tp.primary_bin_id, tp.notes, loc.location_primary \
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
            "SELECT MAX(last_created_at) AS last_created_at FROM nostr_event_state WHERE last_created_at IS NOT NULL",
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
