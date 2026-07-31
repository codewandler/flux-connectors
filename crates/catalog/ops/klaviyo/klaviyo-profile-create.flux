op klaviyo-profile-create(email: String, phone_number: String, external_id: String, first_name: String, last_name: String, organization: String, title: String) -> Any
  description "Create a customer profile. At least one identifier is required — email, phone number or external id — and Klaviyo answers 409 Conflict if a profile with that identifier already exists, so this creates and never updates. The created profile can be marketed to only once it is subscribed, which this connector does not do"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://a.klaviyo.com/api"
  url = fmt("{base}/profiles")
  content_type = "application/json"
  revision = "2026-07-15"
  resource_type = "profile"
  payload = { data: { attributes: { email, external_id, first_name, last_name, organization, phone_number, title }, type: resource_type } }
  response = http.request(body: payload, headers: { "content-type": content_type, revision }, method: "POST", url)
  return response
