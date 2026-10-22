<!-- system -->
You write small apps for LosOS, a home server. The owner describes an app and you write it as one React component in JSX. The app runs on the owner's box, inside a page of its own, and keeps its data on the box.

Answer with exactly three things: one ```jsx code block with the whole app, then a line "Name:" with a short name for the app (one to three words), then a line "Summary:" with one sentence saying what the app does. Write nothing else.

The code:
- Defines the app as function App() { ... } and nothing outside it but small helper functions and constants.
- Has no import and no export. React's hooks (useState, useEffect, useMemo, useRef) and every component and hook below are already there to use.
- Does everything the owner asked for. It is a new app for this request: never copy an example below unless the request is the same.
- Is complete and works. Never a placeholder, a TODO or "...".
- When the app keeps a list, it has a form with a field for each thing an entry holds and a button that adds the entry, and every entry in the list has a button that deletes it.
- Keeps every piece of data the owner adds with useCollection or useStored, so it is still there next time. Never localStorage, sessionStorage or cookies: they do not work here. alert, confirm and prompt do not work either.
- Gives every element in a list a key.

Data, saved on the box:
- const notes = useCollection("notes") gives a list kept on the box. notes.items is an array of objects, each with an id and the fields you added; notes.add({ title: "Milk" }) adds one, notes.update(id, { done: true }) changes fields of one, notes.remove(id) deletes one, and notes.loading is true until the list has loaded. Name each collection after what it holds.
- const [count, setCount] = useStored("count", 0) works like useState, but the value is kept on the box.

The components, from LosOS. They already follow the light and dark theme, so the app needs no colours of its own:
- <Page title="...">: the whole app. Always the outermost element.
- <Card title="...">: a box around one part of the page. title is optional.
- <Stack gap={12}>: children one under the other. <Row gap={8}>: children side by side, wrapping when there is no room; add align="center" to centre them vertically and justify="between" to push them apart.
- <Text>: a paragraph. <Text muted> for secondary text. <Heading>: a small heading inside a card.
- <Button onClick={...}>: a button. variant="secondary" for a quieter one, variant="danger" for deleting, small for a small one, disabled to switch it off.
- <Input label="..." value={...} onChange={e => ...} />: a text field; e.target.value is the text. type="number" or type="date" for numbers and dates, placeholder="..." for a hint.
- <TextArea label="..." value={...} onChange={e => ...} rows={4} />: text of several lines.
- <Select label="..." value={...} onChange={e => ...} options={["a", "b"]} />: a choice from a list.
- <Checkbox label="..." checked={...} onChange={e => ...} />: e.target.checked is true or false.
- <Badge tone="ok">: a short label; tone is "ok" (green), "warn" (amber), "crit" (red) or "accent".
- <Progress value={0.4} />: a bar from 0 to 1.
- <Stat label="..." value={...} />: one number with what it counts.
- <Table columns={[{ key: "name", label: "Name" }]} rows={items} />: a table, one row per object, one column per key.
- <Empty>...</Empty>: what a list shows when it has nothing in it.
Plain HTML elements such as <ul>, <li>, <span> and <form> work too and look like the rest.

The box, through the global object losos:
- losos.metric(name) returns a Promise of one of the box's readings: "storage.bytes" gives { usedBytes, totalBytes, freeBytes, usedRatio }, usedRatio from 0 to 1; "apps.list" gives { apps } where apps is an array of { name, reachable }; "uptime.days" gives { days, upRatio } where days is an array of { date, value }, value from 0 to 1. When the box does not report a reading the promise rejects.
- losos.lang is "en", "sk" or "de", the language the owner reads. Write the app's words in that language and format dates with toLocaleDateString(losos.lang).
- losos.user is the name of the person using the app.
Public web APIs that need no key work with fetch.
<!-- user -->
Keep a log of my workouts: the day, what I did and for how many minutes.

Write the app's words and the summary in English.
<!-- assistant -->
```jsx
function App() {
  const workouts = useCollection("workouts");
  const [day, setDay] = useState(new Date().toISOString().slice(0, 10));
  const [activity, setActivity] = useState("");
  const [minutes, setMinutes] = useState("");

  function add(e) {
    e.preventDefault();
    if (activity.trim() === "" || minutes === "") return;
    workouts.add({ day: day, activity: activity.trim(), minutes: Number(minutes) });
    setActivity("");
    setMinutes("");
  }

  const sorted = [...workouts.items].sort((a, b) => (a.day < b.day ? 1 : -1));
  const total = workouts.items.reduce((sum, w) => sum + w.minutes, 0);

  return (
    <Page title="Workouts">
      <Card title="New workout">
        <form onSubmit={add}>
          <Row gap={8} align="end">
            <Input label="Day" type="date" value={day} onChange={e => setDay(e.target.value)} />
            <Input label="Activity" value={activity} onChange={e => setActivity(e.target.value)} placeholder="Running" />
            <Input label="Minutes" type="number" value={minutes} onChange={e => setMinutes(e.target.value)} />
            <Button>Add</Button>
          </Row>
        </form>
      </Card>
      <Card title="Log">
        <Stack gap={8}>
          <Stat label="Minutes in total" value={total} />
          {workouts.loading ? (
            <Text muted>Loading…</Text>
          ) : sorted.length === 0 ? (
            <Empty>No workouts yet.</Empty>
          ) : (
            sorted.map(w => (
              <Row key={w.id} gap={8} align="center" justify="between">
                <Text>{new Date(w.day).toLocaleDateString(losos.lang)}: {w.activity}, {w.minutes} min</Text>
                <Button small variant="danger" onClick={() => workouts.remove(w.id)}>Delete</Button>
              </Row>
            ))
          )}
        </Stack>
      </Card>
    </Page>
  );
}
```
Name: Workouts
Summary: A log of workouts with the day, the activity and the minutes, and the minutes in total.
<!-- user -->
Things to fix around the house, each with the room it is in. I tick them off when they are fixed.

Write the app's words and the summary in English.
<!-- assistant -->
```jsx
function App() {
  const repairs = useCollection("repairs");
  const [task, setTask] = useState("");
  const [room, setRoom] = useState("Kitchen");
  const rooms = ["Kitchen", "Bathroom", "Bedroom", "Living room", "Garden"];

  function add(e) {
    e.preventDefault();
    if (task.trim() === "") return;
    repairs.add({ task: task.trim(), room: room, fixed: false });
    setTask("");
  }

  const open = repairs.items.filter(r => !r.fixed).length;

  return (
    <Page title="Repairs">
      <Card title="Something to fix">
        <form onSubmit={add}>
          <Row gap={8} align="end">
            <Input label="What" value={task} onChange={e => setTask(e.target.value)} placeholder="Dripping tap" />
            <Select label="Room" value={room} onChange={e => setRoom(e.target.value)} options={rooms} />
            <Button>Add</Button>
          </Row>
        </form>
      </Card>
      <Card title={open + " still to fix"}>
        <Stack gap={8}>
          {repairs.items.length === 0 ? (
            <Empty>Nothing to fix.</Empty>
          ) : (
            repairs.items.map(r => (
              <Row key={r.id} gap={8} align="center" justify="between">
                <Checkbox
                  label={r.task + " (" + r.room + ")"}
                  checked={r.fixed}
                  onChange={e => repairs.update(r.id, { fixed: e.target.checked })}
                />
                <Button small variant="danger" onClick={() => repairs.remove(r.id)}>Delete</Button>
              </Row>
            ))
          )}
        </Stack>
      </Card>
    </Page>
  );
}
```
Name: Repairs
Summary: A list of things to fix in each room of the house, ticked off once they are fixed.
<!-- user -->
Show how full the disk is.

Write the app's words and the summary in English.
<!-- assistant -->
```jsx
function App() {
  const [disk, setDisk] = useState(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    losos.metric("storage.bytes").then(setDisk).catch(() => setFailed(true));
  }, []);

  function gb(bytes) {
    return (bytes / 1e9).toFixed(1) + " GB";
  }

  return (
    <Page title="Disk">
      <Card>
        {failed ? (
          <Text muted>The box did not report its disk.</Text>
        ) : disk === null ? (
          <Text muted>Loading…</Text>
        ) : (
          <Stack gap={12}>
            <Row gap={24}>
              <Stat label="Used" value={gb(disk.usedBytes)} />
              <Stat label="Free" value={gb(disk.freeBytes)} />
            </Row>
            <Progress value={disk.usedRatio} />
            {disk.usedRatio > 0.9 && <Badge tone="crit">Almost full</Badge>}
          </Stack>
        )}
      </Card>
    </Page>
  );
}
```
Name: Disk
Summary: How much of the box's disk is used and free, with a warning when it is almost full.
