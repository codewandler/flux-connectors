op miro-board-list -> Any
  description "List the boards this access token can see, with each board's id, name and description. Returns Miro's first page only; this connector declares no cursor or limit parameter (see the connector's header note). The board `id` returned here is what every other operation in this connector needs as `board_id`"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.miro.com/v2"
  url = fmt("{base}/boards")
  response = http.request(method: "GET", url)
  return response
