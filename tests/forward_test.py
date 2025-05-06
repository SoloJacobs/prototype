from forward import parse_proc_net_unix


def test_parse_proc_net_unix() -> None:
    assert parse_proc_net_unix()
