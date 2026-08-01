op babelforce-list-outbound-attempts(page: Number, max: Number, campaignId: String, listId: String, leadId: String, number: String) -> Any
  description "Get a List of all outbound call attempts (account-wide)"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/attempts")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
    sep = "&"
  when campaignId
    url = fmt("{url}{sep}campaignId={campaignId}")
    sep = "&"
  when listId
    url = fmt("{url}{sep}listId={listId}")
    sep = "&"
  when leadId
    url = fmt("{url}{sep}leadId={leadId}")
    sep = "&"
  when number
    url = fmt("{url}{sep}number={number}")
  response = http.request(method: "GET", url)
  return response
