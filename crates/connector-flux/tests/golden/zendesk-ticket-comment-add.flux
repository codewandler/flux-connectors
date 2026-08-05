op zendesk-ticket-comment-add(ticket_id: Number, updated_stamp: String, body: String, public: Bool) -> Any
  description "Add a comment to a ticket; the comment is an internal note unless public is explicitly true"
  risk "medium"
  idempotency "conditional"
  effects ["write", "network"]
  expose true

  base = "https://example.zendesk.com"
  url = fmt("{base}/api/v2/tickets/{ticket_id}.json")
  content_type = "application/json"
  safe_update = true
  payload = { ticket: { comment: { body, public }, safe_update, updated_stamp } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
