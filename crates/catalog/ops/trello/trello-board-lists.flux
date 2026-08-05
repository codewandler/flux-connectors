op trello-board-lists(id: String) -> Any
  description "List the open lists (columns) on a board, left to right. Each carries the `id` trello-list-cards and trello-card-create take"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.trello.com/1"
  url = fmt("{base}/boards/{id}/lists")
  response = http.request(method: "GET", url)
  return response
