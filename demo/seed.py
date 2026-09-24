#!/usr/bin/env python3
"""Fill a throwaway Linear workspace with demo data for recording.

Creates labels, projects (with milestones), cycles, issues with sub-issues and
threaded comments, favorites, and saved views in one team — enough for every
page of linear-tui to have something to show.

    LINEAR_DEMO_API_KEY=lin_api_... python3 demo/seed.py [--team KEY] [--yes]

Without LINEAR_DEMO_API_KEY it uses the OAuth token from `linear-tui auth
login`, so signing linear-tui in to the demo workspace is enough.

Point it at a workspace made for the demo, never a real one: everything it
creates is visible to the whole workspace. Standard library only.
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request
from datetime import date, datetime, timedelta, timezone

API_URL = "https://api.linear.app/graphql"
MARKER_PROJECT = "Offline Mode"


class Linear:
    def __init__(self, api_key):
        self.api_key = api_key

    def gql(self, query, **variables):
        body = json.dumps({"query": query, "variables": variables}).encode()
        req = urllib.request.Request(
            API_URL,
            data=body,
            headers={"Content-Type": "application/json", "Authorization": self.api_key},
        )
        try:
            with urllib.request.urlopen(req) as resp:
                payload = json.load(resp)
        except urllib.error.HTTPError as e:
            payload = json.loads(e.read() or b"{}")
            if "errors" not in payload:
                raise
        if payload.get("errors"):
            messages = "; ".join(err.get("message", "?") for err in payload["errors"])
            raise RuntimeError(messages)
        return payload["data"]

    def create(self, mutation, input_type, entity, fields, data):
        """Run a `<mutation>(input: ...)` and return the created entity."""
        query = (
            f"mutation($input: {input_type}!) {{ {mutation}(input: $input) "
            f"{{ success {entity} {{ {fields} }} }} }}"
        )
        return self.gql(query, input=data)[mutation][entity]


def iso(days_from_today):
    return (date.today() + timedelta(days=days_from_today)).isoformat()


def timestamp(days_from_today):
    moment = datetime.now(timezone.utc).replace(hour=0, minute=0, second=0, microsecond=0)
    return (moment + timedelta(days=days_from_today)).isoformat()


def login_token():
    """The OAuth access token linear-tui stored, as an Authorization header."""
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config")
    path = os.path.join(base, "linear-tui", "tokens.json")
    try:
        with open(path) as f:
            tokens = json.load(f)
    except FileNotFoundError:
        return None
    if tokens.get("expires_at", 0) <= datetime.now(timezone.utc).timestamp():
        sys.exit("The linear-tui login has expired; run linear-tui once to refresh it.")
    return f"Bearer {tokens['access_token']}"


def warn(message):
    print(f"  ! {message}", file=sys.stderr)


# --- Demo content -----------------------------------------------------------
#
# A fictional weather app, "Nimbus". Issue keys: state is a workflow state
# *type* (backlog / unstarted / started / review / completed / canceled);
# "review" picks a started state named like "In Review" when the team has one.

LABELS = {
    "Bug": "#eb5757",
    "Feature": "#bb87fc",
    "Improvement": "#4ea7fc",
    "Design": "#f2994a",
    "Performance": "#26b5ce",
    "iOS": "#95a2b3",
    "Android": "#4cb782",
}

PROJECTS = [
    {
        "name": MARKER_PROJECT,
        "icon": "Cloud",
        "color": "#4ea7fc",
        "status": "started",
        "start": -21,
        "target": 24,
        "description": "Forecasts that keep working on the subway.",
        "content": (
            "## Why\n\nA third of sessions start with a flaky connection. "
            "Nimbus should show the last forecast instantly and sync when it can.\n\n"
            "## Scope\n\n- Local cache for the 7-day forecast\n"
            "- Background refresh\n- Clear *stale data* indicator\n"
        ),
        "milestones": [("Cache layer", 3), ("Background sync", 14), ("Beta", 24)],
    },
    {
        "name": "Widgets 2.0",
        "icon": "Sun",
        "color": "#f2c94c",
        "status": "planned",
        "start": 10,
        "target": 60,
        "description": "Home screen widgets redesigned from scratch.",
        "content": "Interactive widgets for iOS 18 and Android 15, sharing one layout engine.",
        "milestones": [("Design review", 20), ("Public beta", 50)],
    },
    {
        "name": "Radar Performance",
        "icon": "Lightning",
        "color": "#26b5ce",
        "status": "completed",
        "start": -60,
        "target": -7,
        "description": "Radar map at 60 fps on mid-range phones.",
        "content": "Tile decoding moved off the main thread; frame time down from 34 ms to 11 ms.",
        "milestones": [],
    },
]

# Sub-issues are listed under "children"; comments are (body, replies) pairs.
ISSUES = [
    {
        "title": "Cache the 7-day forecast on device",
        "state": "started",
        "priority": 1,
        "estimate": 5,
        "labels": ["Feature"],
        "project": MARKER_PROJECT,
        "milestone": "Cache layer",
        "cycle": "current",
        "assign": True,
        "due": 3,
        "description": (
            "## Goal\n\nOpen the app offline and see the **last known forecast** "
            "instead of a spinner.\n\n"
            "## Approach\n\n1. Persist the forecast response in SQLite\n"
            "2. Serve from cache first, then revalidate\n"
            "3. Expire entries after 12 hours\n\n"
            "```swift\nlet forecast = try await cache.value(for: location)\n"
            "    ?? api.forecast(for: location)\n```\n\n"
            "> Keep the schema versioned — we will add hourly data later.\n"
        ),
        "children": [
            {"title": "Define the cache schema", "state": "completed", "estimate": 2, "assign": True},
            {"title": "Revalidate on app foreground", "state": "started", "estimate": 2, "assign": True},
            {"title": "Evict entries older than 12 hours", "state": "unstarted", "estimate": 1},
        ],
        "comments": [
            (
                "Should we cache per location or per grid cell? Per cell would dedupe nearby saved places.",
                ["Per cell — the API already rounds coordinates to a 2.5 km grid."],
            ),
            ("Schema PR is up, hourly data can slot in as a second table.", []),
        ],
    },
    {
        "title": "Show a stale-data banner when offline",
        "state": "review",
        "priority": 2,
        "estimate": 2,
        "labels": ["Design", "Feature"],
        "project": MARKER_PROJECT,
        "milestone": "Cache layer",
        "cycle": "current",
        "assign": True,
        "description": (
            "When the forecast is older than an hour, show *Updated 2h ago* above the "
            "hourly strip. Tapping it retries the refresh.\n"
        ),
        "comments": [("Screenshots attached in Figma, using the warning tint from the palette.", [])],
    },
    {
        "title": "Background refresh drains battery on Android",
        "state": "started",
        "priority": 1,
        "estimate": 3,
        "labels": ["Bug", "Android", "Performance"],
        "project": MARKER_PROJECT,
        "milestone": "Background sync",
        "cycle": "current",
        "assign": True,
        "description": (
            "Battery stats show Nimbus waking the device every 5 minutes.\n\n"
            "## Steps to reproduce\n\n- Enable background refresh\n"
            "- Leave the phone idle overnight\n\n"
            "Expected: at most one refresh per hour. Use `WorkManager` with a "
            "network constraint instead of `AlarmManager`.\n"
        ),
        "comments": [("Confirmed on a Pixel 8, 9% overnight vs 2% with refresh off.", [])],
    },
    {
        "title": "Sync saved locations across devices",
        "state": "unstarted",
        "priority": 3,
        "estimate": 5,
        "labels": ["Feature"],
        "project": MARKER_PROJECT,
        "milestone": "Background sync",
        "cycle": "current",
    },
    {
        "title": "Crash when a saved location has no timezone",
        "state": "unstarted",
        "priority": 1,
        "estimate": 1,
        "labels": ["Bug", "iOS"],
        "cycle": "current",
        "assign": True,
        "description": "`TimeZone(identifier:)` returns nil for a few ocean coordinates, and we force-unwrap it.\n",
    },
    {
        "title": "Precipitation chart overlaps the hourly strip on small screens",
        "state": "review",
        "priority": 3,
        "estimate": 1,
        "labels": ["Bug", "Design"],
        "cycle": "current",
        "assign": True,
    },
    {
        "title": "Radar tiles decode on the main thread",
        "state": "completed",
        "priority": 2,
        "estimate": 3,
        "labels": ["Performance"],
        "project": "Radar Performance",
        "assign": True,
    },
    {
        "title": "Prefetch neighbouring radar tiles",
        "state": "completed",
        "priority": 3,
        "estimate": 2,
        "labels": ["Performance", "Improvement"],
        "project": "Radar Performance",
    },
    {
        "title": "Drop frames when zooming the radar map",
        "state": "completed",
        "priority": 2,
        "estimate": 3,
        "labels": ["Bug", "Performance"],
        "project": "Radar Performance",
        "assign": True,
    },
    {
        "title": "Interactive widget layout engine",
        "state": "backlog",
        "priority": 2,
        "estimate": 8,
        "labels": ["Feature", "Design"],
        "project": "Widgets 2.0",
        "milestone": "Design review",
        "description": (
            "## 概要\n\nウィジェットのレイアウトを iOS と Android で共通化する。"
            "現状はプラットフォームごとに別実装になっていて、デザイン変更のたびに二重の作業が発生している。\n\n"
            "- サイズごとのブレークポイントを定義する\n- ダークモードの配色をトークン化する\n"
        ),
    },
    {
        "title": "Lock screen widget for iOS",
        "state": "backlog",
        "priority": 3,
        "estimate": 3,
        "labels": ["Feature", "iOS"],
        "project": "Widgets 2.0",
        "milestone": "Public beta",
    },
    {
        "title": "Material You colours for Android widgets",
        "state": "backlog",
        "priority": 4,
        "estimate": 2,
        "labels": ["Design", "Android"],
        "project": "Widgets 2.0",
    },
    {
        "title": "Add air quality index to the daily summary",
        "state": "unstarted",
        "priority": 3,
        "estimate": 3,
        "labels": ["Feature"],
        "cycle": "next",
    },
    {
        "title": "Localise units for the UK (mph, °C)",
        "state": "unstarted",
        "priority": 4,
        "estimate": 1,
        "labels": ["Improvement"],
        "cycle": "next",
        "assign": True,
    },
    {
        "title": "VoiceOver reads temperatures without units",
        "state": "unstarted",
        "priority": 2,
        "estimate": 1,
        "labels": ["Bug", "iOS"],
        "cycle": "next",
    },
    {
        "title": "Severe weather push notifications",
        "state": "backlog",
        "priority": 2,
        "estimate": 5,
        "labels": ["Feature"],
    },
    {
        "title": "Onboarding asks for location permission too early",
        "state": "backlog",
        "priority": 3,
        "labels": ["Improvement", "Design"],
    },
    {
        "title": "Evaluate a second forecast provider",
        "state": "backlog",
        "priority": 0,
    },
    {
        "title": "Migrate analytics to the new SDK",
        "state": "completed",
        "priority": 4,
        "estimate": 2,
        "labels": ["Improvement"],
        "assign": True,
    },
    {
        "title": "Apple Watch complication",
        "state": "canceled",
        "priority": 4,
        "labels": ["Feature", "iOS"],
    },
]

FAVORITE_ISSUE = "Cache the 7-day forecast on device"


# --- Seeding ----------------------------------------------------------------


def pick_team(linear, key):
    teams = linear.gql("{ teams { nodes { id key name cyclesEnabled } } }")["teams"]["nodes"]
    if not teams:
        sys.exit("The workspace has no teams.")
    if key is None:
        return teams[0]
    for team in teams:
        if team["key"].lower() == key.lower():
            return team
    sys.exit(f"No team with key {key!r}; found {', '.join(t['key'] for t in teams)}.")


def states_by_kind(linear, team_id):
    nodes = linear.gql(
        "query($id: String!) { team(id: $id) { states { nodes { id name type position } } } }",
        id=team_id,
    )["team"]["states"]["nodes"]
    nodes.sort(key=lambda s: s["position"])
    kinds = {}
    for state in nodes:
        kinds.setdefault(state["type"], state["id"])
    review = next(
        (s["id"] for s in nodes if s["type"] == "started" and "review" in s["name"].lower()),
        None,
    )
    kinds["review"] = review or kinds.get("started")
    # A started state that is not the review one, so "In Progress" is not
    # swallowed by "In Review" when the latter sorts first.
    in_progress = next(
        (s["id"] for s in nodes if s["type"] == "started" and s["id"] != review),
        kinds.get("started"),
    )
    kinds["started"] = in_progress
    return kinds


def ensure_labels(linear, team_id):
    existing = linear.gql("{ issueLabels(first: 250) { nodes { id name team { id } } } }")
    ids = {}
    for label in existing["issueLabels"]["nodes"]:
        if label["team"] is None or label["team"]["id"] == team_id:
            ids[label["name"]] = label["id"]
    for name, color in LABELS.items():
        if name in ids:
            continue
        created = linear.create(
            "issueLabelCreate", "IssueLabelCreateInput", "issueLabel", "id",
            {"name": name, "color": color, "teamId": team_id},
        )
        ids[name] = created["id"]
        print(f"  label     {name}")
    return ids


def create_projects(linear, team_id, viewer_id):
    statuses = linear.gql("{ projectStatuses { nodes { id type } } }")["projectStatuses"]["nodes"]
    status_ids = {}
    for status in statuses:
        status_ids.setdefault(status["type"], status["id"])

    projects, milestones = {}, {}
    for spec in PROJECTS:
        data = {
            "name": spec["name"],
            "icon": spec["icon"],
            "color": spec["color"],
            "description": spec["description"],
            "content": spec["content"],
            "teamIds": [team_id],
            "leadId": viewer_id,
            "startDate": iso(spec["start"]),
            "targetDate": iso(spec["target"]),
        }
        if spec["status"] in status_ids:
            data["statusId"] = status_ids[spec["status"]]
        try:
            project = linear.create("projectCreate", "ProjectCreateInput", "project", "id", data)
        except RuntimeError as e:
            # Icon names are validated server-side; retry without one.
            warn(f"project {spec['name']}: {e}; retrying without icon")
            data.pop("icon")
            project = linear.create("projectCreate", "ProjectCreateInput", "project", "id", data)
        projects[spec["name"]] = project["id"]
        print(f"  project   {spec['name']}")
        for name, target in spec["milestones"]:
            milestone = linear.create(
                "projectMilestoneCreate", "ProjectMilestoneCreateInput", "projectMilestone", "id",
                {"name": name, "projectId": project["id"], "targetDate": iso(target)},
            )
            milestones[(spec["name"], name)] = milestone["id"]
    return projects, milestones


def ensure_cycles(linear, team):
    """Return {"current": id, "next": id}, enabling cycles if needed."""
    if not team["cyclesEnabled"]:
        try:
            linear.gql(
                "mutation($id: String!, $input: TeamUpdateInput!) "
                "{ teamUpdate(id: $id, input: $input) { success } }",
                id=team["id"],
                input={"cyclesEnabled": True, "cycleDuration": 2, "upcomingCycleCount": 2},
            )
            print("  cycles    enabled (2-week cycles)")
        except RuntimeError as e:
            warn(f"enabling cycles: {e}; issues will have no cycle")
            return {}

    def fetch():
        return linear.gql(
            "query($id: String!) { team(id: $id) { activeCycle { id } "
            "cycles(first: 20) { nodes { id startsAt isFuture } } } }",
            id=team["id"],
        )["team"]

    result = fetch()
    current = (result["activeCycle"] or {}).get("id")
    future = sorted(
        (c for c in result["cycles"]["nodes"] if c["isFuture"]), key=lambda c: c["startsAt"]
    )
    if current is None and future:
        # Linear generates cycles itself once they are enabled (and then
        # refuses cycleCreate), starting on the next cycle start day. A start
        # in the past is rejected too, so the first one starts in two minutes.
        try:
            linear.gql(
                "mutation($id: String!, $input: CycleUpdateInput!) "
                "{ cycleUpdate(id: $id, input: $input) { success } }",
                id=future[0]["id"],
                input={"startsAt": (datetime.now(timezone.utc) + timedelta(minutes=2)).isoformat()},
            )
            current = future[0]["id"]
            result = fetch()
            print("  cycles    current cycle starts in two minutes")
        except RuntimeError as e:
            warn(f"moving the first cycle: {e}")
    if current is None:
        try:
            cycle = linear.create(
                "cycleCreate", "CycleCreateInput", "cycle", "id",
                {"teamId": team["id"], "startsAt": timestamp(-4), "endsAt": timestamp(10)},
            )
        except RuntimeError as e:
            warn(f"current cycle: {e}; issues will have no cycle")
            return {}
        current = cycle["id"]
        result = fetch()
    future = sorted(
        (c for c in result["cycles"]["nodes"] if c["isFuture"]), key=lambda c: c["startsAt"]
    )
    upcoming = future[0]["id"] if future else None
    if upcoming is None:
        try:
            upcoming = linear.create(
                "cycleCreate", "CycleCreateInput", "cycle", "id",
                {"teamId": team["id"], "startsAt": timestamp(10), "endsAt": timestamp(24)},
            )["id"]
        except RuntimeError as e:
            warn(f"next cycle: {e}")
    return {"current": current, "next": upcoming}


def create_issue(linear, spec, ctx, parent_id=None, project_name=None, cycle_name=None):
    data = {
        "teamId": ctx["team_id"],
        "title": spec["title"],
        "priority": spec.get("priority", 0),
        "stateId": ctx["states"][spec["state"]],
    }
    if parent_id:
        data["parentId"] = parent_id
    if "description" in spec:
        data["description"] = spec["description"]
    if "estimate" in spec:
        data["estimate"] = spec["estimate"]
    if spec.get("assign"):
        data["assigneeId"] = ctx["viewer_id"]
    if "due" in spec:
        data["dueDate"] = iso(spec["due"])
    if spec.get("labels"):
        data["labelIds"] = [ctx["labels"][name] for name in spec["labels"]]
    project_name = spec.get("project", project_name)
    if project_name:
        data["projectId"] = ctx["projects"][project_name]
        if "milestone" in spec:
            data["projectMilestoneId"] = ctx["milestones"][(project_name, spec["milestone"])]
    cycle_name = spec.get("cycle", cycle_name)
    cycle = ctx["cycles"].get(cycle_name)
    if cycle:
        data["cycleId"] = cycle

    try:
        issue = linear.create("issueCreate", "IssueCreateInput", "issue", "id identifier", data)
    except RuntimeError as e:
        # Estimates are rejected when the team has estimation turned off.
        if "estimate" not in data:
            raise
        warn(f"{spec['title']}: {e}; retrying without estimate")
        data.pop("estimate")
        issue = linear.create("issueCreate", "IssueCreateInput", "issue", "id identifier", data)
    print(f"  issue     {issue['identifier']}  {spec['title']}")

    for body, replies in spec.get("comments", []):
        comment = linear.create(
            "commentCreate", "CommentCreateInput", "comment", "id",
            {"issueId": issue["id"], "body": body},
        )
        for reply in replies:
            linear.create(
                "commentCreate", "CommentCreateInput", "comment", "id",
                {"issueId": issue["id"], "body": reply, "parentId": comment["id"]},
            )
    for child in spec.get("children", []):
        create_issue(
            linear, child, ctx, parent_id=issue["id"], project_name=project_name, cycle_name=cycle_name
        )
    return issue


def create_views(linear, team_id, label_ids):
    views = {}
    specs = [
        (
            "Urgent bugs",
            {"teamId": team_id, "icon": "Bug", "color": "#eb5757",
             "filterData": {"labels": {"id": {"eq": label_ids["Bug"]}}, "priority": {"in": [1, 2]}}},
        ),
        (
            "Mobile polish",
            {"icon": "Sparkle", "color": "#f2994a",
             "filterData": {"labels": {"id": {"in": [label_ids["Design"], label_ids["Improvement"]]}}}},
        ),
        (
            "Active projects",
            {"icon": "Cube", "color": "#4ea7fc",
             "projectFilterData": {"status": {"type": {"eq": "started"}}}},
        ),
    ]
    for name, data in specs:
        data = {"name": name, "shared": True, **data}
        try:
            try:
                view = linear.create("customViewCreate", "CustomViewCreateInput", "customView", "id", data)
            except RuntimeError:
                data.pop("icon", None)
                view = linear.create("customViewCreate", "CustomViewCreateInput", "customView", "id", data)
            views[name] = view["id"]
            print(f"  view      {name}")
        except RuntimeError as e:
            warn(f"view {name}: {e}")
    return views


def create_favorites(linear, entries):
    for label, data in entries:
        try:
            linear.create("favoriteCreate", "FavoriteCreateInput", "favorite", "id", data)
            print(f"  favorite  {label}")
        except RuntimeError as e:
            warn(f"favorite {label}: {e}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--team", help="team key to seed (default: the first team)")
    parser.add_argument("--yes", action="store_true", help="skip the confirmation prompt")
    args = parser.parse_args()

    auth = os.environ.get("LINEAR_DEMO_API_KEY") or login_token()
    if not auth:
        sys.exit("Set LINEAR_DEMO_API_KEY, or sign linear-tui in to the demo workspace.")
    linear = Linear(auth)

    me = linear.gql("{ viewer { id name organization { name urlKey } } }")["viewer"]
    org = me["organization"]
    team = pick_team(linear, args.team)
    print(f"Workspace: {org['name']} (linear.app/{org['urlKey']})")
    print(f"Team:      {team['name']} ({team['key']})")
    print(f"User:      {me['name']}")

    existing = linear.gql(
        'query($name: String!) { projects(filter: { name: { eq: $name } }) { nodes { id } } }',
        name=MARKER_PROJECT,
    )["projects"]["nodes"]
    if existing:
        sys.exit(f"Project {MARKER_PROJECT!r} already exists — this workspace looks seeded already.")

    if not args.yes:
        answer = input(f"Type the workspace URL key ({org['urlKey']}) to create demo data there: ")
        if answer.strip() != org["urlKey"]:
            sys.exit("Aborted.")

    print("Seeding...")
    labels = ensure_labels(linear, team["id"])
    projects, milestones = create_projects(linear, team["id"], me["id"])
    cycles = ensure_cycles(linear, team)
    ctx = {
        "team_id": team["id"],
        "viewer_id": me["id"],
        "states": states_by_kind(linear, team["id"]),
        "labels": labels,
        "projects": projects,
        "milestones": milestones,
        "cycles": cycles,
    }
    issues = {spec["title"]: create_issue(linear, spec, ctx) for spec in ISSUES}
    views = create_views(linear, team["id"], labels)

    favorites = [
        (MARKER_PROJECT, {"projectId": projects[MARKER_PROJECT]}),
        ("Widgets 2.0", {"projectId": projects["Widgets 2.0"]}),
        (FAVORITE_ISSUE, {"issueId": issues[FAVORITE_ISSUE]["id"]}),
    ]
    if cycles.get("current"):
        favorites.append(("Current cycle", {"cycleId": cycles["current"]}))
    if "Urgent bugs" in views:
        favorites.append(("Urgent bugs", {"customViewId": views["Urgent bugs"]}))
    create_favorites(linear, favorites)

    print(f"Done. Set default_team = \"{team['name']}\" when recording.")


if __name__ == "__main__":
    try:
        main()
    except RuntimeError as e:
        sys.exit(f"Linear API error: {e}")
