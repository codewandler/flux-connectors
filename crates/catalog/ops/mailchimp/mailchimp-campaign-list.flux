op mailchimp-campaign-list -> Any
  description "List the campaigns in the account — drafts, scheduled, sending and sent alike. Returns Mailchimp's default first page only, and this connector declares no status or date filter, so compare the number of entries against `total_items` before treating the result as complete"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{dc}.api.mailchimp.com/3.0"
  url = fmt("{base}/campaigns")
  response = http.request(method: "GET", url)
  return response
