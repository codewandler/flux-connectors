op klaviyo-list-list -> Any
  description "List the account's lists — static, opt-in collections of profiles, as opposed to segments, which are live queries and are not shipped here. Returns the FIRST PAGE ONLY; Klaviyo's `page[cursor]` query parameter cannot be sent by this connector. Returns the lists themselves, not their members"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://a.klaviyo.com/api"
  url = fmt("{base}/lists")
  revision = "2026-07-15"
  response = http.request(headers: { revision }, method: "GET", url)
  return response
