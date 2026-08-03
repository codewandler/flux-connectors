op freshdesk-contact-list(phone: String, email: String, mobile: String, company_id: String, state: String) -> Any
  description "List and filter contacts, e.g. to resolve a caller before filing a ticket"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{domain}/api/v2"
  url = fmt("{base}/contacts")
  response = http.request(method: "GET", query: { company_id, email, mobile, phone, state }, url)
  return response
