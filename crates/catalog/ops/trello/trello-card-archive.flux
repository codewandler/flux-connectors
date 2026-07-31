op trello-card-archive(id: String, closed: Bool) -> Any
  description "Archive a card, or restore an archived one. Archiving is reversible and destroys nothing: the card leaves its list and stays readable in the board's archive"
  risk "medium"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.trello.com/1"
  url = fmt("{base}/cards/{id}")
  content_type = "application/json"
  payload = { closed }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
