op zendesk-ticket-update(ticket_id: Number, ticket: Any) -> Any
  description "Update Ticket"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/tickets/{ticket_id}")
  content_type = "application/json"
  payload = { ticket }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
