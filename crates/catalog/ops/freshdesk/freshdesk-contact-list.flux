op freshdesk-contact-list(phone: String, email: String, mobile: String, company_id: String, state: String) -> Any
  description "List and filter contacts, e.g. to resolve a caller before filing a ticket"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{domain}/api/v2"
  url = fmt("{base}/contacts")
  sep = "?"
  when phone
    url = fmt("{url}{sep}phone={phone}")
    sep = "&"
  when email
    url = fmt("{url}{sep}email={email}")
    sep = "&"
  when mobile
    url = fmt("{url}{sep}mobile={mobile}")
    sep = "&"
  when company_id
    url = fmt("{url}{sep}company_id={company_id}")
    sep = "&"
  when state
    url = fmt("{url}{sep}state={state}")
  response = http.request(method: "GET", url)
  return response
