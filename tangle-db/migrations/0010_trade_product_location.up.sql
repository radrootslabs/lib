CREATE TABLE IF NOT EXISTS trade_product_location (
    tb_tp CHAR(36),
    tb_gl CHAR(36),
    FOREIGN KEY (tb_tp) REFERENCES trade_product(id) ON DELETE CASCADE,
    FOREIGN KEY (tb_gl) REFERENCES gcs_location(id) ON DELETE CASCADE,
    PRIMARY KEY (tb_tp, tb_gl)
);
