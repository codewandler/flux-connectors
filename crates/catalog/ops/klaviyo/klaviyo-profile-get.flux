op klaviyo-profile-get(id: String) -> Any
  description "Read one customer profile by its Klaviyo id, with its identifiers, custom properties, consent state and location. Returns personal data about a named individual"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://a.klaviyo.com/api"
  url = fmt("{base}/profiles/{id}")
  revision = "2026-07-15"
  response = http.request(headers: { revision }, method: "GET", url)
  return response
