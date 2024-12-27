import React, { ReactEventHandler, useRef, useState } from 'react';
import { CircularProgress } from '@mui/material';
import { grey } from '@mui/material/colors';
import { BrokenImage } from '@mui/icons-material';
import { Box } from '@mui/system';
import NextImage from 'next/image';

const Image = ({
  src,
  alt,
  onLoad,
  onError,
  animationDuration = 3000,
  aspectRatio = 1,
  ...image
}: {
  src: string;
  alt: string;
  layout?: string;
  onLoad?: Function;
  onError?: Function;
  animationDuration?: number;
  aspectRatio?: number;
}) => {
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState(false);
  const imageRef = useRef<HTMLImageElement>(null);

  const handleLoadImage: ReactEventHandler<HTMLImageElement> = (e) => {
    setLoaded(true);
    setError(false);
    if (onLoad) {
      onLoad(e);
    }
  };

  const handleImageError: ReactEventHandler<HTMLImageElement> = (e) => {
    if (src) {
      setError(true);
      if (onError) {
        onError(e);
      }
    }
  };

  const imageTransition = {
    opacity: loaded ? 1 : 0,
    filterBrightness: loaded ? 100 : 0,
    filterSaturate: loaded ? 100 : 20,
    transition: `
        filterBrightness ${
          animationDuration * 0.75
        }ms cubic-bezier(0.4, 0.0, 0.2, 1),
        filterSaturate ${animationDuration}ms cubic-bezier(0.4, 0.0, 0.2, 1),
        opacity ${animationDuration / 2}ms cubic-bezier(0.4, 0.0, 0.2, 1)`,
  };

  return (
    <Box
      sx={{
        paddingTop: `calc(1 / ${aspectRatio} * 100%)`,
        position: 'relative',
      }}
    >
      {src && (
        <NextImage
          {...image}
          src={src}
          alt={alt}
          ref={imageRef}
          style={{
            borderRadius: '20px',
            width: '100%',
            height: '100%',
            position: 'absolute',
            objectFit: 'cover',
            cursor: 'inherit',
            top: 0,
            left: 0,
            ...imageTransition,
          }}
          fill
          onLoad={handleLoadImage}
          onError={handleImageError}
        />
      )}
      <Box
        style={{
          width: '100%',
          height: '100%',
          position: 'absolute',
          cursor: 'inherit',
          top: 0,
          left: 0,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          pointerEvents: 'none',
        }}
      >
        {!loaded && !error && <CircularProgress size={48} />}
        {error && (
          <BrokenImage sx={{ width: 48, height: 48, color: grey[300] }} />
        )}
      </Box>
    </Box>
  );
};

export default Image;
