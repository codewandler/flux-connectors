op hubspot-deal-get(deal_id: Number) -> Any
  description "Read one deal by record id. Returns only HubSpot's default deal properties — name, amount, stage, pipeline and close date. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/category` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.hubapi.com"
  url = fmt("{base}/crm/v3/objects/deals/{deal_id}")
  response = http.request(method: "GET", url)
  return response
