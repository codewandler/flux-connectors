op miro-sticky-note-update(board_id: String, item_id: String, content: String) -> Any
  description "Update a sticky note's text content. Setting the same content twice ends in the same state, but this is declared non_idempotent because check_write_metadata refuses idempotency = idempotent on any PATCH by method alone, regardless of vendor behaviour (see the header note, C-186). The updated note is in the response"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.miro.com/v2"
  url = fmt("{base}/boards/{board_id}/sticky_notes/{item_id}")
  content_type = "application/json"
  payload = { data: { content } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PATCH", url)
  return response
