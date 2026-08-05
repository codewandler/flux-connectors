op freshdesk-ticket-list(req_id: String, req_email: String, company_id: String, updated: String) -> Any
  description "List and filter tickets"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{domain}/api/v2"
  url = fmt("{base}/tickets")
  response = http.request(method: "GET", query: { company_id, email: req_email, requester_id: req_id, updated_since: updated }, url)
  return response
