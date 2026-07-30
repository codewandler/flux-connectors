op zendesk-ticket-comment-list(ticket_id: Number, page: Number, per_page: Number) -> Any
  description "List a ticket's comments"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://{subdomain}.zendesk.com"
  $url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")
  $sep = "?"
  when $page
    $url = fmt("{url}{sep}page={page}")
    $sep = "&"
  when $per_page
    $url = fmt("{url}{sep}per_page={per_page}")
  $response = http.request({ method: "GET", url: $url })
  return $response
