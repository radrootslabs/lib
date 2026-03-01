CREATE TABLE IF NOT EXISTS trade_product_media (
    tb_tp CHAR(36),
    tb_mu CHAR(36),
    FOREIGN KEY (tb_tp) REFERENCES trade_product(id) ON DELETE CASCADE,
    FOREIGN KEY (tb_mu) REFERENCES media_image(id) ON DELETE CASCADE,
    PRIMARY KEY (tb_tp, tb_mu)
);