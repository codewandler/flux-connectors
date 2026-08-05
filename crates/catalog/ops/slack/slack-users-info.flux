op slack-users-info(user: String, include_locale: Bool) -> Any
  description "Look up one Slack user by id — display name, real name, time zone and whether they are a bot. Slack answers HTTP 200 even on failure: check `ok` in the response body, where an error such as `user_not_found` appears at `error`. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://slack.com"
  url = fmt("{base}/api/users.info")
  content_type = "application/json"
  payload = { include_locale, user }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
