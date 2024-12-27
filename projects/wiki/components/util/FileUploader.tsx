import { UploadFile } from '@mui/icons-material';
import { Button, CircularProgress, Input } from '@mui/material';
import { Box } from '@mui/system';
import { nhost } from 'nhost';
import { ChangeEventHandler, useState } from 'react';

const FileUploader = ({
  text,
  onNewFile,
}: {
  text: string;
  onNewFile: ({ fileId, file }: { fileId?: string; file: File }) => void;
}) => {
  const [loading, setLoading] = useState(false);
  const handleChangeFile: ChangeEventHandler<HTMLInputElement> = async (e) => {
    setLoading(true);
    const file = e.target.files?.[0];
    if (file) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const res = (await nhost.storage.upload({ file })) as any;
      console.log(res)
      onNewFile({ fileId: res.fileMetadata?.processedFiles?.[0]?.id ?? res.fileMetadata?.id, file });
    }
    setLoading(false);
  };

  return (
    <Box sx={{ position: 'relative', width: '100%' }}>
      <Input
        id="input-file"
        type="file"
        onChange={handleChangeFile}
        sx={{ display: 'none' }}
      />
      <label htmlFor="input-file">
        <Button
          color="secondary"
          component="span"
          variant="outlined"
          startIcon={<UploadFile />}
          disabled={loading}
        >
          {text}
        </Button>
      </label>
      {loading && (
        <CircularProgress
          size={24}
          sx={{
            position: 'absolute',
            top: '50%',
            left: '50%',
            marginTop: '-12px',
            marginLeft: '-12px',
          }}
        />
      )}
    </Box>
  );
};

export default FileUploader;
