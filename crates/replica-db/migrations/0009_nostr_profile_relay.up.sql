CREATE TABLE IF NOT EXISTS nostr_profile_relay (
    tb_pr CHAR(36),
    tb_rl CHAR(36),
    FOREIGN KEY (tb_pr) REFERENCES nostr_profile(id) ON DELETE CASCADE,
    FOREIGN KEY (tb_rl) REFERENCES nostr_relay(id) ON DELETE CASCADE,
    PRIMARY KEY (tb_pr, tb_rl)
);