op klaviyo-event-create(metric_name: String, profile_email: String, properties: Any, value: Number, value_currency: String, unique_id: String) -> Any
  description "Post an event about a customer — a purchase, a page view, anything the account measures. THIS CAN CAUSE A MESSAGE TO BE SENT: an event triggers any Klaviyo flow built on its metric, so an email or SMS may reach the named profile immediately. Klaviyo creates the metric and the profile on first use if they do not exist. Answers 202 Accepted with no body: the event is queued, not confirmed. Pass unique_id to make a retry safe — Klaviyo records only the first event with a given unique_id for the same profile and metric"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://a.klaviyo.com/api"
  url = fmt("{base}/events")
  content_type = "application/json"
  revision = "2026-07-15"
  resource_type = "event"
  metric_type = "metric"
  profile_type = "profile"
  payload = { data: { attributes: { metric: { data: { attributes: { name: metric_name }, type: metric_type } }, profile: { data: { attributes: { email: profile_email }, type: profile_type } }, properties, unique_id, value, value_currency }, type: resource_type } }
  response = http.request(body: payload, headers: { "content-type": content_type, revision }, method: "POST", url)
  return response
