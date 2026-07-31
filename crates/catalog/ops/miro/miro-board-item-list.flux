op miro-board-item-list(board_id: String) -> Any
  description "List the items on a board, of any type (sticky note, shape, text or frame). Each item's shape depends on its `type`: sticky notes and text carry `data.content`, shapes carry `data.content` and `data.shape`, frames carry `data.title`. Returns Miro's first page only; this connector declares no cursor or limit parameter (see the connector's header note)"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.miro.com/v2"
  url = fmt("{base}/boards/{board_id}/items")
  response = http.request(method: "GET", url)
  return response
