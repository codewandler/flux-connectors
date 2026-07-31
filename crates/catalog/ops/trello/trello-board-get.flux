op trello-board-get(id: String) -> Any
  description "Get one board by id, with its settings and current state"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.trello.com/1"
  url = fmt("{base}/boards/{id}")
  response = http.request(method: "GET", url)
  return response
