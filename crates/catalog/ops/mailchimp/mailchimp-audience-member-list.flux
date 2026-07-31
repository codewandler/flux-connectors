op mailchimp-audience-member-list(list_id: String) -> Any
  description "List the contacts in an audience, subscribed and unsubscribed alike. Returns Mailchimp's default first page only — this connector declares no paging or status filter, so compare the number of entries against `total_items` before treating the result as complete. Every entry is personal data"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{dc}.api.mailchimp.com/3.0"
  url = fmt("{base}/lists/{list_id}/members")
  response = http.request(method: "GET", url)
  return response
