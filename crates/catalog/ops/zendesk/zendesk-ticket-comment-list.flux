op zendesk-ticket-comment-list(ticket_id: Number, include_inline_images: Bool, per_page: Number) -> Any
  description "List Comments"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/tickets/{ticket_id}/comments")
  response = http.request(method: "GET", query: { include_inline_images, per_page }, url)
  return response
