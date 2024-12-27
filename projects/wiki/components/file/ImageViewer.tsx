import { Box } from '@mui/system';
import { Image } from 'comps';

const ImageViewer = ({ file }: { file?: string }) =>
  file ? (
    <Box sx={{ p: 1, m: 1 }}>
      <Image src={file} alt="" layout="fill" />
    </Box>
  ) : null;

export default ImageViewer;
