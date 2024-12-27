import { read, utils } from 'xlsx';
import { Input } from '@mui/material';
import { ChangeEventHandler } from 'react';

const SheetReader = ({
  children,
  onFileLoaded,
}: {
  children: JSX.Element;
  onFileLoaded: Function;
}) => {
  const handleChangeFile: ChangeEventHandler<HTMLInputElement> = (e) => {
    const reader = new FileReader();
    const files = e.target.files;

    if (files?.length) {
      // eslint-disable-next-line functional/immutable-data
      reader.onload = () => {
        const wb = read(reader.result);
        const data = utils.sheet_to_json(wb.Sheets[wb.SheetNames[0]]);

        onFileLoaded(data ?? []);
      };

      reader.readAsArrayBuffer(files[0]);
    }
  };

  return (
    <>
      <Input
        id="input-file"
        type="file"
        onChange={handleChangeFile}
        sx={{ display: 'none' }}
      />
      <label htmlFor="input-file">{children}</label>
    </>
  );
};

export default SheetReader;
