op freshdesk-ticket-list(req_id: String, req_email: String, company_id: String, updated: String) -> Any
  description "List and filter tickets"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{domain}/api/v2"
  url = fmt("{base}/tickets")
  sep = "?"
  when req_id
    url = fmt("{url}{sep}requester_id={req_id}")
    sep = "&"
  when req_email
    url = fmt("{url}{sep}email={req_email}")
    sep = "&"
  when company_id
    url = fmt("{url}{sep}company_id={company_id}")
    sep = "&"
  when updated
    url = fmt("{url}{sep}updated_since={updated}")
  response = http.request(method: "GET", url)
  return response
