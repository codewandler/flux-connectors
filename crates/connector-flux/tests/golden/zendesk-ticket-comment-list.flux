op zendesk-ticket-comment-list(ticket_id: Number, page: Number, per_page: Number) -> Any
  description "List one Zendesk ticket's comments."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://example.zendesk.com"
  url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")
  response = http.request(method: "GET", query: { page, per_page }, url)
  return response
