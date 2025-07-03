import { Button } from '@mui/material';

const Crash = () => {
  return <Button onClick={() => { throw Error("Triggered Crash") }}>Trigger Crash</Button>;
};

export default Crash;
