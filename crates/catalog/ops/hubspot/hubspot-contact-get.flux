op hubspot-contact-get(contact_id: Number) -> Any
  description "Read one contact by record id. Returns only HubSpot's default contact properties — name, email and record timestamps; a custom property needs a `properties` projection this connector cannot express yet. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/category` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.hubapi.com"
  url = fmt("{base}/crm/v3/objects/contacts/{contact_id}")
  response = http.request(method: "GET", url)
  return response
