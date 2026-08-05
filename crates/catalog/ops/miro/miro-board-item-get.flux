op miro-board-item-get(board_id: String, item_id: String) -> Any
  description "Get one item from a board, of any type (sticky note, shape, text or frame). Its shape depends on its `type` — see miro-board-item-list's description for what each carries"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.miro.com/v2"
  url = fmt("{base}/boards/{board_id}/items/{item_id}")
  response = http.request(method: "GET", url)
  return response
