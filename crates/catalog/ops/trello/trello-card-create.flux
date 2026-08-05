op trello-card-create(list_id: String, name: String, description: String) -> Any
  description "Create a card at the bottom of a list. The card is created unassigned, with no due date and no labels; Trello notifies whoever watches the list"
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.trello.com/1"
  url = fmt("{base}/cards")
  content_type = "application/json"
  payload = { desc: description, idList: list_id, name }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
