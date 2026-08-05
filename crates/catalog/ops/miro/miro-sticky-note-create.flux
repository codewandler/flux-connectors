op miro-sticky-note-create(board_id: String, content: String) -> Any
  description "Create a sticky note on a board. Miro does not deduplicate: creating the same content twice makes two sticky notes, so this is not idempotent. The created note, with its assigned id, is in the response"
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.miro.com/v2"
  url = fmt("{base}/boards/{board_id}/sticky_notes")
  content_type = "application/json"
  payload = { data: { content } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
