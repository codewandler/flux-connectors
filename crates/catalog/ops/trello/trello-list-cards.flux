op trello-list-cards(id: String) -> Any
  description "List the open cards in one list, in board order. Archived cards are not returned"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.trello.com/1"
  url = fmt("{base}/lists/{id}/cards")
  response = http.request(method: "GET", url)
  return response
