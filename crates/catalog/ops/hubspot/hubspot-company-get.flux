op hubspot-company-get(company_id: Number) -> Any
  description "Read one company by record id. Returns only HubSpot's default company properties — name, domain and record timestamps. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/category` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.hubapi.com"
  url = fmt("{base}/crm/v3/objects/companies/{company_id}")
  response = http.request(method: "GET", url)
  return response
