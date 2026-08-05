op mailchimp-audience-member-upsert(list_id: String, subscriber_hash: String, email_address: String, status_if_new: String, status: String) -> Any
  description "Add a contact to an audience, or update the one already there. Creating with `status_if_new = \"pending\"` makes Mailchimp send its own opt-in confirmation and record the consent; creating with \"subscribed\" asserts that the account already holds that consent and sends nothing. Setting `status` on an existing contact changes whether they receive mail. This writes personal data about a third party and is not undone by calling it again"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{dc}.api.mailchimp.com/3.0"
  url = fmt("{base}/lists/{list_id}/members/{subscriber_hash}")
  content_type = "application/json"
  payload = { email_address, status, status_if_new }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
