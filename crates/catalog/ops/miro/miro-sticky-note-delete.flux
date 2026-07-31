op miro-sticky-note-delete(board_id: String, item_id: String) -> Any
  description "Delete one sticky note. There is no undo route in the API. Responds with no content"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.miro.com/v2"
  url = fmt("{base}/boards/{board_id}/sticky_notes/{item_id}")
  response = http.request(method: "DELETE", url)
  return response
