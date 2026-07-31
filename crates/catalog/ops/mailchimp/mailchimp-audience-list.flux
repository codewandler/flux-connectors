op mailchimp-audience-list -> Any
  description "List the audiences in the account, with the id every contact operation takes. Returns Mailchimp's default first page only — this connector declares no paging parameters, so compare the number of entries against `total_items` before treating the result as complete"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{dc}.api.mailchimp.com/3.0"
  url = fmt("{base}/lists")
  response = http.request(method: "GET", url)
  return response
