op zendesk-ticket-comment-list(ticket_id: Number, include_inline_images: Bool, per_page: Number) -> Any
  description "List Comments"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/tickets/{ticket_id}/comments")
  sep = "?"
  when include_inline_images
    url = fmt("{url}{sep}include_inline_images={include_inline_images}")
    sep = "&"
  when per_page
    url = fmt("{url}{sep}per_page={per_page}")
  response = http.request(method: "GET", url)
  return response
