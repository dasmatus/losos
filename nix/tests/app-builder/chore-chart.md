```jsx
function App() {
  const chores = useCollection("chores");
  const [task, setTask] = useState("");
  const [child, setChild] = useState("");
  const children = ["Alice", "Bob", "Charlie", "David"];

  function add(e) {
    e.preventDefault();
    if (task.trim() === "" || child.trim() === "") return;
    chores.add({ task: task.trim(), child: child.trim(), done: false });
    setTask("");
    setChild("");
  }

  const completed = chores.items.filter(c => c.done).length;

  return (
    <Page title="Chores">
      <Card title="Chore chart">
        <form onSubmit={add}>
          <Row gap={8} align="end">
            <Input label="Task" value={task} onChange={e => setTask(e.target.value)} placeholder="Clean the room" />
            <Select label="Child" value={child} onChange={e => setChild(e.target.value)} options={children} />
            <Button>Add</Button>
          </Row>
        </form>
      </Card>
      <Card title={completed + " completed"}>
        <Stack gap={8}>
          {chores.items.length === 0 ? (
            <Empty>Nothing to do.</Empty>
          ) : (
            chores.items.map(c => (
              <Row key={c.id} gap={8} align="center" justify="between">
                <Checkbox
                  label={c.task + " for " + c.child}
                  checked={c.done}
                  onChange={e => chores.update(c.id, { done: e.target.checked })}
                />
                <Button small variant="danger" onClick={() => chores.remove(c.id)}>Delete</Button>
              </Row>
            ))
          )}
        </Stack>
      </Card>
    </Page>
  );
}
```
Name: Chores
Summary: A list of chores for the kids, ticked off once they are completed.
