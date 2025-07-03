import { Button } from '@mui/material';

const Crash = () => {
  // eslint-disable-next-line functional/no-throw-statements
  return <Button onClick={() => { throw Error("Triggered Crash") }}>Trigger Crash</Button>;
};

export default Crash;
